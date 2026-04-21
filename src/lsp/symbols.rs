//! Symbol provider for LSP document and workspace symbols
//!
//! Provides hierarchical document symbols and workspace symbol search.

use std::path::Path;

use tower_lsp::lsp_types::*;

use crate::lsp::cache::{CachedClass, CachedFunction, CachedSymbol, FileAnalysis, LspCache};

/// Symbol provider for document and workspace symbols
pub struct SymbolProvider;

impl SymbolProvider {
    /// Get hierarchical document symbols for a file
    #[allow(deprecated)]
    pub fn document_symbols(cache: &LspCache, path: &Path) -> Option<Vec<DocumentSymbol>> {
        let file = cache.get_file(path)?;
        let mut symbols = Vec::new();

        // Add module as root symbol (optional - can be removed if not desired)
        // For now, we just add classes and functions directly

        // Add classes with their methods as children
        for class in &file.classes {
            symbols.push(Self::class_to_document_symbol(class));
        }

        // Add top-level functions
        for func in &file.functions {
            symbols.push(Self::function_to_document_symbol(func, SymbolKind::FUNCTION));
        }

        Some(symbols)
    }

    /// Convert a cached class to a hierarchical DocumentSymbol
    #[allow(deprecated)]
    fn class_to_document_symbol(class: &CachedClass) -> DocumentSymbol {
        let children: Vec<DocumentSymbol> = class
            .methods
            .iter()
            .map(|m| Self::function_to_document_symbol(m, SymbolKind::METHOD))
            .collect();

        DocumentSymbol {
            name: class.name.clone(),
            detail: class.docstring.as_ref().map(|d| {
                // Truncate docstring for detail view
                let first_line = d.lines().next().unwrap_or("");
                if first_line.len() > 80 {
                    format!("{}...", &first_line[..77])
                } else {
                    first_line.to_string()
                }
            }),
            kind: SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: class.line,
                    character: 0,
                },
                end: Position {
                    line: class.end_line,
                    character: 0,
                },
            },
            selection_range: Range {
                start: Position {
                    line: class.line,
                    character: 0,
                },
                end: Position {
                    line: class.line,
                    character: class.name.len() as u32 + 6, // "class " prefix
                },
            },
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        }
    }

    /// Convert a cached function to a DocumentSymbol
    #[allow(deprecated)]
    fn function_to_document_symbol(func: &CachedFunction, kind: SymbolKind) -> DocumentSymbol {
        DocumentSymbol {
            name: func.name.clone(),
            detail: Some(func.signature.clone()),
            kind,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: func.line,
                    character: 0,
                },
                end: Position {
                    line: func.end_line,
                    character: 0,
                },
            },
            selection_range: Range {
                start: Position {
                    line: func.line,
                    character: 0,
                },
                end: Position {
                    line: func.line,
                    character: func.name.len() as u32 + 4, // "def " prefix
                },
            },
            children: None,
        }
    }

    /// Get workspace symbols matching a query
    #[allow(deprecated)]
    pub fn workspace_symbols(cache: &LspCache, query: &str) -> Vec<SymbolInformation> {
        cache.get_workspace_symbols(query)
    }

    /// Map a CachedSymbol to SymbolInformation
    #[allow(deprecated)]
    pub fn cached_to_symbol_info(symbol: &CachedSymbol) -> Option<SymbolInformation> {
        let uri = Url::from_file_path(&symbol.path).ok()?;

        Some(SymbolInformation {
            name: symbol.name.clone(),
            kind: symbol.kind,
            tags: None,
            deprecated: None,
            location: Location {
                uri,
                range: Range {
                    start: Position {
                        line: symbol.line,
                        character: 0,
                    },
                    end: Position {
                        line: symbol.end_line,
                        character: 0,
                    },
                },
            },
            container_name: symbol.container.clone(),
        })
    }

    /// Get the appropriate SymbolKind for a given entity type
    pub fn get_symbol_kind(entity_type: &str) -> SymbolKind {
        match entity_type.to_lowercase().as_str() {
            "class" | "struct" => SymbolKind::CLASS,
            "function" | "func" | "fn" => SymbolKind::FUNCTION,
            "method" => SymbolKind::METHOD,
            "module" | "namespace" => SymbolKind::MODULE,
            "enum" => SymbolKind::ENUM,
            "interface" | "trait" => SymbolKind::INTERFACE,
            "constant" | "const" => SymbolKind::CONSTANT,
            "variable" | "var" | "let" => SymbolKind::VARIABLE,
            "property" | "field" => SymbolKind::PROPERTY,
            "constructor" => SymbolKind::CONSTRUCTOR,
            "event" => SymbolKind::EVENT,
            "operator" => SymbolKind::OPERATOR,
            "type" | "typedef" | "type_alias" => SymbolKind::TYPE_PARAMETER,
            "package" => SymbolKind::PACKAGE,
            _ => SymbolKind::NULL,
        }
    }

    /// Create hierarchical symbols from file analysis including nested structures
    #[allow(deprecated)]
    pub fn create_hierarchical_symbols(file: &FileAnalysis) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();

        // Add classes with their methods as children
        for class in &file.classes {
            symbols.push(Self::class_to_document_symbol(class));
        }

        // Add top-level functions
        for func in &file.functions {
            symbols.push(Self::function_to_document_symbol(func, SymbolKind::FUNCTION));
        }

        symbols
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::cache::LspCache;
    use std::path::PathBuf;

    #[test]
    fn test_get_symbol_kind() {
        assert_eq!(SymbolProvider::get_symbol_kind("class"), SymbolKind::CLASS);
        assert_eq!(SymbolProvider::get_symbol_kind("struct"), SymbolKind::CLASS);
        assert_eq!(
            SymbolProvider::get_symbol_kind("function"),
            SymbolKind::FUNCTION
        );
        assert_eq!(SymbolProvider::get_symbol_kind("method"), SymbolKind::METHOD);
        assert_eq!(SymbolProvider::get_symbol_kind("enum"), SymbolKind::ENUM);
        assert_eq!(
            SymbolProvider::get_symbol_kind("interface"),
            SymbolKind::INTERFACE
        );
        assert_eq!(
            SymbolProvider::get_symbol_kind("constant"),
            SymbolKind::CONSTANT
        );
        assert_eq!(SymbolProvider::get_symbol_kind("unknown"), SymbolKind::NULL);
    }

    #[test]
    fn test_function_to_document_symbol() {
        let func = CachedFunction {
            name: "my_func".to_string(),
            docstring: Some("A test function".to_string()),
            line: 10,
            end_line: 20,
            signature: "def my_func(x: int) -> str".to_string(),
            line_count: 11,
        };

        let symbol = SymbolProvider::function_to_document_symbol(&func, SymbolKind::FUNCTION);

        assert_eq!(symbol.name, "my_func");
        assert_eq!(symbol.kind, SymbolKind::FUNCTION);
        assert_eq!(symbol.range.start.line, 10);
        assert_eq!(symbol.range.end.line, 20);
        assert!(symbol.children.is_none());
    }

    #[test]
    fn test_class_to_document_symbol() {
        let method = CachedFunction {
            name: "method1".to_string(),
            docstring: None,
            line: 15,
            end_line: 18,
            signature: "def method1(self)".to_string(),
            line_count: 4,
        };

        let class = CachedClass {
            name: "MyClass".to_string(),
            docstring: Some("A test class".to_string()),
            line: 10,
            end_line: 30,
            methods: vec![method],
        };

        let symbol = SymbolProvider::class_to_document_symbol(&class);

        assert_eq!(symbol.name, "MyClass");
        assert_eq!(symbol.kind, SymbolKind::CLASS);
        assert_eq!(symbol.range.start.line, 10);
        assert_eq!(symbol.range.end.line, 30);
        assert!(symbol.children.is_some());
        assert_eq!(symbol.children.as_ref().unwrap().len(), 1);
        assert_eq!(symbol.children.as_ref().unwrap()[0].name, "method1");
    }

    #[test]
    fn test_document_symbols_no_file() {
        let cache = LspCache::new();
        let result = SymbolProvider::document_symbols(&cache, Path::new("/nonexistent.py"));
        assert!(result.is_none());
    }

    #[test]
    fn test_workspace_symbols_empty() {
        let cache = LspCache::new();
        let symbols = SymbolProvider::workspace_symbols(&cache, "test");
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_cached_to_symbol_info() {
        let symbol = CachedSymbol {
            name: "TestClass".to_string(),
            kind: SymbolKind::CLASS,
            path: PathBuf::from("/test/file.py"),
            line: 5,
            end_line: 25,
            container: None,
        };

        let info = SymbolProvider::cached_to_symbol_info(&symbol);
        assert!(info.is_some());

        let info = info.unwrap();
        assert_eq!(info.name, "TestClass");
        assert_eq!(info.kind, SymbolKind::CLASS);
        assert_eq!(info.location.range.start.line, 5);
    }

    #[test]
    fn test_class_docstring_truncation() {
        let class = CachedClass {
            name: "MyClass".to_string(),
            docstring: Some("This is a very long docstring that should be truncated because it exceeds the maximum length we want to show in the detail view of the symbol outline.".to_string()),
            line: 10,
            end_line: 30,
            methods: vec![],
        };

        let symbol = SymbolProvider::class_to_document_symbol(&class);

        // Detail should be truncated to ~80 chars with "..."
        if let Some(detail) = symbol.detail {
            assert!(detail.len() <= 83); // 80 + "..."
            assert!(detail.ends_with("..."));
        }
    }
}
