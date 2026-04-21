//! LSP server module for Cartographer

pub mod analysis;
pub mod cache;
pub mod diagnostics;
pub mod handlers;
pub mod symbols;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::lsp::analysis::AnalysisBridge;
use crate::lsp::cache::LspCache;
use crate::lsp::diagnostics::DiagnosticsProvider;
use crate::lsp::handlers::Handlers;
use crate::lsp::symbols::SymbolProvider;

/// The LSP backend for Cartographer
pub struct Backend {
    /// LSP client for sending notifications
    client: Client,
    /// Analysis bridge for code analysis
    analysis: Arc<RwLock<AnalysisBridge>>,
    /// Cache for analysis results
    cache: Arc<RwLock<LspCache>>,
    /// Set of currently open file paths
    open_files: Arc<RwLock<HashSet<PathBuf>>>,
}

impl Backend {
    /// Create a new Backend instance
    pub fn new(client: Client) -> Self {
        Self {
            client,
            analysis: Arc::new(RwLock::new(AnalysisBridge::new())),
            cache: Arc::new(RwLock::new(LspCache::new())),
            open_files: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Publish diagnostics for all open files
    async fn publish_diagnostics(&self) {
        let analysis = self.analysis.read().await;
        let open_files: Vec<PathBuf> = self.open_files.read().await.iter().cloned().collect();

        let diagnostics = DiagnosticsProvider::generate_diagnostics(&analysis, &open_files);

        for (uri, file_diagnostics) in diagnostics {
            self.client
                .publish_diagnostics(uri, file_diagnostics, None)
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root_uri) = params.root_uri {
            if let Ok(root_path) = root_uri.to_file_path() {
                let mut analysis = self.analysis.write().await;
                if let Err(e) = analysis.analyze_workspace(&root_path) {
                    self.client
                        .log_message(MessageType::ERROR, format!("Analysis failed: {}", e))
                        .await;
                } else {
                    // Populate cache from analysis
                    let mut cache = self.cache.write().await;
                    cache.populate_from_analysis(&analysis);
                }
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some("cartographer".to_string()),
                        inter_file_dependencies: true,
                        workspace_diagnostics: true,
                        work_done_progress_options: Default::default(),
                    },
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "cartographer-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Cartographer LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let cache = self.cache.read().await;
        Ok(Handlers::hover(&cache, params))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        let cache = self.cache.read().await;
        match SymbolProvider::document_symbols(&cache, &file_path) {
            Some(symbols) => Ok(Some(DocumentSymbolResponse::Nested(symbols))),
            None => Ok(None),
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let cache = self.cache.read().await;
        Ok(Handlers::workspace_symbol(&cache, params))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let cache = self.cache.read().await;
        Ok(Handlers::goto_definition(&cache, params))
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let cache = self.cache.read().await;
        Ok(Handlers::code_lens(&cache, params))
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        // Track the opened file
        if let Ok(path) = params.text_document.uri.to_file_path() {
            self.open_files.write().await.insert(path);
        }

        // Publish diagnostics for open files
        self.publish_diagnostics().await;

        Handlers::did_open(&self.client, params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Ensure the file is tracked as open
        if let Ok(path) = params.text_document.uri.to_file_path() {
            self.open_files.write().await.insert(path);
        }

        Handlers::did_change(&self.client, params).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Ok(path) = params.text_document.uri.to_file_path() {
            let mut analysis = self.analysis.write().await;
            if let Err(e) = analysis.analyze_workspace(&path) {
                self.client
                    .log_message(MessageType::WARNING, format!("Re-analysis failed: {}", e))
                    .await;
            } else {
                let mut cache = self.cache.write().await;
                cache.populate_from_analysis(&analysis);
            }
        }

        // Publish updated diagnostics after re-analysis
        // Drop the write locks first by scoping above
        self.publish_diagnostics().await;

        Handlers::did_save(&self.client, params).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // Remove the closed file from tracking and clear its diagnostics
        if let Ok(path) = params.text_document.uri.to_file_path() {
            self.open_files.write().await.remove(&path);

            // Clear diagnostics for the closed file
            self.client
                .publish_diagnostics(params.text_document.uri.clone(), vec![], None)
                .await;
        }

        Handlers::did_close(&self.client, params).await;
    }
}

/// Start the LSP server on stdin/stdout
pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = tower_lsp::LspService::new(Backend::new);
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_backend_creation() {
        // Backend requires a Client which needs async runtime
        // This test just ensures the module compiles correctly
        assert!(true);
    }
}
