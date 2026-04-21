//! LSP request handlers

use tower_lsp::lsp_types::*;
use tower_lsp::Client;

use crate::lsp::cache::LspCache;

/// Handlers for LSP requests
pub struct Handlers;

impl Handlers {
    /// Handle hover request
    pub fn hover(cache: &LspCache, params: HoverParams) -> Option<Hover> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let file_path = uri.to_file_path().ok()?;
        let info = cache.get_hover_info(&file_path, position)?;

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: info,
            }),
            range: None,
        })
    }

    /// Handle document symbol request
    pub fn document_symbol(
        cache: &LspCache,
        params: DocumentSymbolParams,
    ) -> Option<DocumentSymbolResponse> {
        let uri = params.text_document.uri;
        let file_path = uri.to_file_path().ok()?;

        let symbols = cache.get_document_symbols(&file_path)?;
        Some(DocumentSymbolResponse::Flat(symbols))
    }

    /// Handle workspace symbol request
    pub fn workspace_symbol(
        cache: &LspCache,
        params: WorkspaceSymbolParams,
    ) -> Option<Vec<SymbolInformation>> {
        let query = &params.query;
        Some(cache.get_workspace_symbols(query))
    }

    /// Handle goto definition request
    pub fn goto_definition(
        cache: &LspCache,
        params: GotoDefinitionParams,
    ) -> Option<GotoDefinitionResponse> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let file_path = uri.to_file_path().ok()?;
        let location = cache.get_definition(&file_path, position)?;

        Some(GotoDefinitionResponse::Scalar(location))
    }

    /// Handle code lens request
    pub fn code_lens(cache: &LspCache, params: CodeLensParams) -> Option<Vec<CodeLens>> {
        let uri = params.text_document.uri;
        let file_path = uri.to_file_path().ok()?;

        cache.get_code_lenses(&file_path)
    }

    /// Handle diagnostics (called after analysis)
    pub fn publish_diagnostics(
        _cache: &LspCache,
        _uri: &Url,
    ) -> Vec<Diagnostic> {
        // Stub: return empty diagnostics
        Vec::new()
    }

    /// Handle did_open notification
    pub async fn did_open(client: &Client, params: DidOpenTextDocumentParams) {
        client
            .log_message(
                MessageType::INFO,
                format!("Opened: {}", params.text_document.uri),
            )
            .await;
    }

    /// Handle did_change notification
    pub async fn did_change(client: &Client, params: DidChangeTextDocumentParams) {
        client
            .log_message(
                MessageType::INFO,
                format!("Changed: {}", params.text_document.uri),
            )
            .await;
    }

    /// Handle did_save notification
    pub async fn did_save(client: &Client, params: DidSaveTextDocumentParams) {
        client
            .log_message(
                MessageType::INFO,
                format!("Saved: {}", params.text_document.uri),
            )
            .await;
    }

    /// Handle did_close notification
    pub async fn did_close(client: &Client, params: DidCloseTextDocumentParams) {
        client
            .log_message(
                MessageType::INFO,
                format!("Closed: {}", params.text_document.uri),
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::cache::{CachedClass, CachedFunction, FileAnalysis};
    use crate::analysis::Language;
    use std::path::PathBuf;

    #[test]
    fn test_handlers_struct_exists() {
        // Basic test to ensure handlers module compiles
        let _ = Handlers;
    }

    #[test]
    fn test_publish_diagnostics_stub() {
        let cache = LspCache::new();
        let uri = Url::parse("file:///test.py").unwrap();
        let diagnostics = Handlers::publish_diagnostics(&cache, &uri);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_goto_definition_no_file() {
        let cache = LspCache::new();
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::parse("file:///nonexistent.py").unwrap(),
                },
                position: Position { line: 0, character: 0 },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = Handlers::goto_definition(&cache, params);
        assert!(result.is_none());
    }

    #[test]
    fn test_hover_returns_info() {
        let mut cache = LspCache::new();
        let path = PathBuf::from("/test/hover.py");
        let uri = Url::from_file_path(&path).unwrap();

        let file = FileAnalysis {
            module_name: "hover".to_string(),
            path: path.clone(),
            description: Some("Test module".to_string()),
            language: Some(Language::Python),
            dependency_count: 2,
            classes: vec![],
            functions: vec![CachedFunction {
                name: "test_func".to_string(),
                docstring: Some("A test function".to_string()),
                line: 5,
                end_line: 10,
                signature: "def test_func(x: int) -> str".to_string(),
                line_count: 6,
            }],
        };

        // Insert directly into cache for testing
        cache.files.insert(path.to_string_lossy().to_string(), file);

        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line: 6, character: 0 },
            },
            work_done_progress_params: Default::default(),
        };

        let result = Handlers::hover(&cache, params);
        assert!(result.is_some());
        let hover = result.unwrap();
        match hover.contents {
            HoverContents::Markup(markup) => {
                assert!(markup.value.contains("test_func"));
            }
            _ => panic!("Expected markup content"),
        }
    }

    #[test]
    fn test_workspace_symbol_empty_query() {
        let cache = LspCache::new();
        let params = WorkspaceSymbolParams {
            query: "".to_string(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = Handlers::workspace_symbol(&cache, params);
        assert!(result.is_some());
        assert!(result.unwrap().is_empty());
    }
}
