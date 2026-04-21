//! LSP analysis cache
//!
//! Provides thread-safe caching for analysis results used by the LSP server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::*;

use crate::analysis::{ClassNode, FunctionNode, Language, Module};
use crate::lsp::analysis::AnalysisBridge;

/// Cached analysis data for a single file
#[derive(Debug, Clone)]
pub struct FileAnalysis {
    /// Module name
    pub module_name: String,
    /// File path
    pub path: PathBuf,
    /// File description (docstring)
    pub description: Option<String>,
    /// Detected language
    pub language: Option<Language>,
    /// Number of dependencies (imports)
    pub dependency_count: usize,
    /// Classes in this file
    pub classes: Vec<CachedClass>,
    /// Functions in this file
    pub functions: Vec<CachedFunction>,
}

/// Cached class data
#[derive(Debug, Clone)]
pub struct CachedClass {
    /// Class name
    pub name: String,
    /// Docstring
    pub docstring: Option<String>,
    /// Line number (0-indexed)
    pub line: u32,
    /// End line
    pub end_line: u32,
    /// Methods in this class
    pub methods: Vec<CachedFunction>,
}

/// Cached function data
#[derive(Debug, Clone)]
pub struct CachedFunction {
    /// Function name
    pub name: String,
    /// Docstring
    pub docstring: Option<String>,
    /// Line number (0-indexed)
    pub line: u32,
    /// End line
    pub end_line: u32,
    /// Signature
    pub signature: String,
    /// Number of lines (basic complexity indicator)
    pub line_count: usize,
}

/// Symbol information for workspace symbol search
#[derive(Debug, Clone)]
pub struct CachedSymbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: SymbolKind,
    /// File path
    pub path: PathBuf,
    /// Line number (0-indexed)
    pub line: u32,
    /// End line
    pub end_line: u32,
    /// Container name (e.g., class name for methods)
    pub container: Option<String>,
}

/// Reference to a symbol (e.g., an import or function call)
#[derive(Debug, Clone)]
pub struct SymbolReference {
    /// The name being referenced
    pub name: String,
    /// File containing this reference
    pub file_path: PathBuf,
    /// Line of the reference
    pub line: u32,
    /// Column start of the reference
    pub column_start: u32,
    /// Column end of the reference
    pub column_end: u32,
    /// The target symbol name (for imports that rename)
    pub target_name: Option<String>,
}

/// LSP cache for analysis results
#[derive(Debug, Default)]
pub struct LspCache {
    /// Cached file analysis data, keyed by file path string
    pub files: HashMap<String, FileAnalysis>,
    /// All symbols for workspace symbol search
    symbols: Vec<CachedSymbol>,
    /// Module information
    modules: Vec<Module>,
    /// Symbol name to definition location lookup
    symbol_definitions: HashMap<String, CachedSymbol>,
}

impl LspCache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self::default()
    }

    /// Populate cache from analysis results
    pub fn populate_from_analysis(&mut self, analysis: &AnalysisBridge) {
        self.files.clear();
        self.symbols.clear();
        self.modules.clear();
        self.symbol_definitions.clear();

        let Some(result) = analysis.result() else {
            return;
        };

        // Cache modules
        self.modules = result.modules.clone();

        // Cache file data
        for (file_id, file_node) in result.graph.all_files() {
            let path_str = file_node.path.to_string_lossy().to_string();
            let language = file_node
                .path
                .extension()
                .and_then(|e| e.to_str())
                .and_then(Language::from_extension);

            let mut classes = Vec::new();
            let mut functions = Vec::new();

            // Cache classes
            for class_id in &file_node.classes {
                if let Some(class_node) = result.graph.get_class(*class_id) {
                    let cached_class = self.cache_class(class_node, &result.graph);

                    // Add class symbol
                    let class_symbol = CachedSymbol {
                        name: class_node.name.clone(),
                        kind: SymbolKind::CLASS,
                        path: file_node.path.clone(),
                        line: class_node.line_start.saturating_sub(1) as u32,
                        end_line: class_node.line_end.saturating_sub(1) as u32,
                        container: None,
                    };
                    self.symbol_definitions
                        .insert(class_node.name.clone(), class_symbol.clone());
                    self.symbols.push(class_symbol);

                    // Add method symbols
                    for method in &cached_class.methods {
                        let method_symbol = CachedSymbol {
                            name: method.name.clone(),
                            kind: SymbolKind::METHOD,
                            path: file_node.path.clone(),
                            line: method.line,
                            end_line: method.end_line,
                            container: Some(class_node.name.clone()),
                        };
                        // Store method with qualified name (ClassName.method_name)
                        let qualified_name =
                            format!("{}.{}", class_node.name, method.name);
                        self.symbol_definitions
                            .insert(qualified_name, method_symbol.clone());
                        self.symbols.push(method_symbol);
                    }

                    classes.push(cached_class);
                }
            }

            // Cache top-level functions
            for func_id in &file_node.functions {
                if let Some(func_node) = result.graph.get_function(*func_id) {
                    let cached_func = Self::cache_function(func_node);

                    // Add function symbol
                    let func_symbol = CachedSymbol {
                        name: func_node.name.clone(),
                        kind: SymbolKind::FUNCTION,
                        path: file_node.path.clone(),
                        line: cached_func.line,
                        end_line: cached_func.end_line,
                        container: None,
                    };
                    self.symbol_definitions
                        .insert(func_node.name.clone(), func_symbol.clone());
                    self.symbols.push(func_symbol);

                    functions.push(cached_func);
                }
            }

            // Also add module name as a symbol for cross-file navigation
            let module_symbol = CachedSymbol {
                name: file_node.module_name.clone(),
                kind: SymbolKind::MODULE,
                path: file_node.path.clone(),
                line: 0,
                end_line: 0,
                container: None,
            };
            self.symbol_definitions
                .insert(file_node.module_name.clone(), module_symbol);

            // Count dependencies
            let dependency_count = result.graph.imports_of(file_id).len();

            self.files.insert(
                path_str,
                FileAnalysis {
                    module_name: file_node.module_name.clone(),
                    path: file_node.path.clone(),
                    description: file_node.docstring.clone(),
                    language,
                    dependency_count,
                    classes,
                    functions,
                },
            );
        }
    }

    fn cache_class(
        &self,
        class_node: &ClassNode,
        graph: &crate::analysis::CodeGraph,
    ) -> CachedClass {
        let methods: Vec<CachedFunction> = class_node
            .methods
            .iter()
            .filter_map(|func_id| graph.get_function(*func_id))
            .map(Self::cache_function)
            .collect();

        CachedClass {
            name: class_node.name.clone(),
            docstring: class_node.docstring.clone(),
            line: class_node.line_start.saturating_sub(1) as u32,
            end_line: class_node.line_end.saturating_sub(1) as u32,
            methods,
        }
    }

    fn cache_function(func_node: &FunctionNode) -> CachedFunction {
        let line_count = func_node.line_end.saturating_sub(func_node.line_start) + 1;
        CachedFunction {
            name: func_node.name.clone(),
            docstring: func_node.docstring.clone(),
            line: func_node.line_start.saturating_sub(1) as u32,
            end_line: func_node.line_end.saturating_sub(1) as u32,
            signature: func_node.signature.clone(),
            line_count,
        }
    }

    /// Get hover info for a position in a file
    pub fn get_hover_info(&self, path: &Path, position: Position) -> Option<String> {
        let path_str = path.to_string_lossy().to_string();
        let file = self.files.get(&path_str)?;
        let line = position.line;

        // Check if we're on a class
        for class in &file.classes {
            if line >= class.line && line <= class.end_line {
                // Check if we're on a method
                for method in &class.methods {
                    if line >= method.line && line <= method.end_line {
                        return Some(Self::format_function_hover(method, Some(&class.name)));
                    }
                }

                // We're on the class itself
                return Some(Self::format_class_hover(class, file.dependency_count));
            }
        }

        // Check if we're on a function
        for func in &file.functions {
            if line >= func.line && line <= func.end_line {
                return Some(Self::format_function_hover(func, None));
            }
        }

        // Return module info if not on any symbol
        Some(Self::format_module_hover(file))
    }

    /// Format hover info for a module
    fn format_module_hover(file: &FileAnalysis) -> String {
        let lang_str = file
            .language
            .map(|l| format!("{:?}", l))
            .unwrap_or_else(|| "Unknown".to_string());

        let mut info = format!("## Module: {}\n\n", file.module_name);
        info.push_str("| Property | Value |\n");
        info.push_str("|----------|-------|\n");
        info.push_str(&format!("| Language | {} |\n", lang_str));
        info.push_str(&format!("| Dependencies | {} |\n", file.dependency_count));
        info.push_str(&format!("| Classes | {} |\n", file.classes.len()));
        info.push_str(&format!("| Functions | {} |\n", file.functions.len()));

        if let Some(desc) = &file.description {
            if !desc.trim().is_empty() {
                info.push_str(&format!("\n---\n\n{}", desc));
            }
        }

        info
    }

    /// Format hover info for a class
    fn format_class_hover(class: &CachedClass, file_import_count: usize) -> String {
        let mut info = format!("## class {}\n\n", class.name);
        info.push_str("| Property | Value |\n");
        info.push_str("|----------|-------|\n");
        info.push_str(&format!("| Methods | {} |\n", class.methods.len()));
        info.push_str(&format!("| Lines | {} |\n", class.end_line.saturating_sub(class.line) + 1));
        info.push_str(&format!("| File imports | {} |\n", file_import_count));

        if let Some(doc) = &class.docstring {
            if !doc.trim().is_empty() {
                info.push_str(&format!("\n---\n\n{}", doc));
            }
        }

        info
    }

    /// Format hover info for a function
    fn format_function_hover(func: &CachedFunction, container: Option<&str>) -> String {
        let mut info = format!("```\n{}\n```\n\n", func.signature);

        info.push_str("| Property | Value |\n");
        info.push_str("|----------|-------|\n");
        info.push_str(&format!("| Lines | {} |\n", func.line_count));

        // Add complexity indicator based on line count
        let complexity = if func.line_count <= 10 {
            "Low"
        } else if func.line_count <= 30 {
            "Medium"
        } else {
            "High"
        };
        info.push_str(&format!("| Complexity | {} |\n", complexity));

        if let Some(container_name) = container {
            info.push_str(&format!("| Container | {} |\n", container_name));
        }

        if let Some(doc) = &func.docstring {
            if !doc.trim().is_empty() {
                info.push_str(&format!("\n---\n\n{}", doc));
            }
        }

        info
    }

    /// Get definition location for a symbol at a position
    pub fn get_definition(&self, path: &Path, position: Position) -> Option<Location> {
        let path_str = path.to_string_lossy().to_string();
        let file = self.files.get(&path_str)?;
        let line = position.line;

        // Check if we're on a class definition
        for class in &file.classes {
            if line >= class.line && line <= class.end_line {
                // If we're on a method within the class
                for method in &class.methods {
                    if line >= method.line && line <= method.end_line {
                        return Some(Location {
                            uri: Url::from_file_path(path).ok()?,
                            range: Range {
                                start: Position {
                                    line: method.line,
                                    character: 0,
                                },
                                end: Position {
                                    line: method.line,
                                    character: 0,
                                },
                            },
                        });
                    }
                }

                // We're on the class itself
                return Some(Location {
                    uri: Url::from_file_path(path).ok()?,
                    range: Range {
                        start: Position {
                            line: class.line,
                            character: 0,
                        },
                        end: Position {
                            line: class.line,
                            character: 0,
                        },
                    },
                });
            }
        }

        // Check functions
        for func in &file.functions {
            if line >= func.line && line <= func.end_line {
                return Some(Location {
                    uri: Url::from_file_path(path).ok()?,
                    range: Range {
                        start: Position {
                            line: func.line,
                            character: 0,
                        },
                        end: Position {
                            line: func.line,
                            character: 0,
                        },
                    },
                });
            }
        }

        None
    }

    /// Get definition location by symbol name (for cross-file navigation)
    pub fn get_definition_by_name(&self, name: &str) -> Option<Location> {
        // Try exact match first
        if let Some(symbol) = self.symbol_definitions.get(name) {
            return Some(Location {
                uri: Url::from_file_path(&symbol.path).ok()?,
                range: Range {
                    start: Position {
                        line: symbol.line,
                        character: 0,
                    },
                    end: Position {
                        line: symbol.line,
                        character: 0,
                    },
                },
            });
        }

        // Try finding in symbols list (case-insensitive fallback)
        for symbol in &self.symbols {
            if symbol.name.eq_ignore_ascii_case(name) {
                return Some(Location {
                    uri: Url::from_file_path(&symbol.path).ok()?,
                    range: Range {
                        start: Position {
                            line: symbol.line,
                            character: 0,
                        },
                        end: Position {
                            line: symbol.line,
                            character: 0,
                        },
                    },
                });
            }
        }

        None
    }

    /// Get all symbols (for testing)
    #[cfg(test)]
    pub fn all_symbols(&self) -> &[CachedSymbol] {
        &self.symbols
    }

    /// Get symbol definitions map (for testing)
    #[cfg(test)]
    pub fn symbol_definitions(&self) -> &HashMap<String, CachedSymbol> {
        &self.symbol_definitions
    }

    /// Find definition by symbol name across the workspace
    pub fn find_definition_by_name(&self, symbol_name: &str) -> Option<Location> {
        for symbol in &self.symbols {
            if symbol.name == symbol_name {
                return Some(Location {
                    uri: Url::from_file_path(&symbol.path).ok()?,
                    range: Range {
                        start: Position {
                            line: symbol.line,
                            character: 0,
                        },
                        end: Position {
                            line: symbol.line,
                            character: 0,
                        },
                    },
                });
            }
        }
        None
    }

    /// Get document symbols for a file
    pub fn get_document_symbols(&self, path: &Path) -> Option<Vec<SymbolInformation>> {
        let path_str = path.to_string_lossy().to_string();
        let file = self.files.get(&path_str)?;
        let uri = Url::from_file_path(path).ok()?;

        let mut symbols = Vec::new();

        // Add classes
        for class in &file.classes {
            #[allow(deprecated)]
            symbols.push(SymbolInformation {
                name: class.name.clone(),
                kind: SymbolKind::CLASS,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
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
                },
                container_name: None,
            });

            // Add methods
            for method in &class.methods {
                #[allow(deprecated)]
                symbols.push(SymbolInformation {
                    name: method.name.clone(),
                    kind: SymbolKind::METHOD,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: method.line,
                                character: 0,
                            },
                            end: Position {
                                line: method.end_line,
                                character: 0,
                            },
                        },
                    },
                    container_name: Some(class.name.clone()),
                });
            }
        }

        // Add functions
        for func in &file.functions {
            #[allow(deprecated)]
            symbols.push(SymbolInformation {
                name: func.name.clone(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
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
                },
                container_name: None,
            });
        }

        Some(symbols)
    }

    /// Get workspace symbols matching a query
    pub fn get_workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        let query_lower = query.to_lowercase();

        self.symbols
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&query_lower))
            .filter_map(|s| {
                let uri = Url::from_file_path(&s.path).ok()?;
                #[allow(deprecated)]
                Some(SymbolInformation {
                    name: s.name.clone(),
                    kind: s.kind,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri,
                        range: Range {
                            start: Position {
                                line: s.line,
                                character: 0,
                            },
                            end: Position {
                                line: s.end_line,
                                character: 0,
                            },
                        },
                    },
                    container_name: s.container.clone(),
                })
            })
            .collect()
    }

    /// Get code lenses for a file
    pub fn get_code_lenses(&self, path: &Path) -> Option<Vec<CodeLens>> {
        let path_str = path.to_string_lossy().to_string();
        let file = self.files.get(&path_str)?;

        let mut lenses = Vec::new();

        // Add lens for each class showing method count
        for class in &file.classes {
            lenses.push(CodeLens {
                range: Range {
                    start: Position {
                        line: class.line,
                        character: 0,
                    },
                    end: Position {
                        line: class.line,
                        character: 0,
                    },
                },
                command: Some(Command {
                    title: format!("{} methods", class.methods.len()),
                    command: String::new(),
                    arguments: None,
                }),
                data: None,
            });
        }

        Some(lenses)
    }

    /// Get file analysis
    pub fn get_file(&self, path: &Path) -> Option<&FileAnalysis> {
        let path_str = path.to_string_lossy().to_string();
        self.files.get(&path_str)
    }

    /// Get all modules
    pub fn modules(&self) -> &[Module] {
        &self.modules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_cache_new() {
        let cache = LspCache::new();
        assert!(cache.files.is_empty());
        assert!(cache.symbols.is_empty());
        assert!(cache.modules.is_empty());
    }

    #[test]
    fn test_file_analysis() {
        let file = FileAnalysis {
            module_name: "test".to_string(),
            path: PathBuf::from("test.py"),
            description: Some("Test module".to_string()),
            language: Some(Language::Python),
            dependency_count: 2,
            classes: vec![],
            functions: vec![],
        };

        assert_eq!(file.module_name, "test");
        assert_eq!(file.dependency_count, 2);
    }

    #[test]
    fn test_cached_symbol() {
        let symbol = CachedSymbol {
            name: "MyClass".to_string(),
            kind: SymbolKind::CLASS,
            path: PathBuf::from("test.py"),
            line: 10,
            end_line: 20,
            container: None,
        };

        assert_eq!(symbol.name, "MyClass");
        assert_eq!(symbol.kind, SymbolKind::CLASS);
    }

    #[test]
    fn test_get_workspace_symbols_empty() {
        let cache = LspCache::new();
        let symbols = cache.get_workspace_symbols("test");
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_get_hover_info_no_file() {
        let cache = LspCache::new();
        let result = cache.get_hover_info(Path::new("nonexistent.py"), Position::default());
        assert!(result.is_none());
    }

    #[test]
    fn test_get_definition_no_file() {
        let cache = LspCache::new();
        let result = cache.get_definition(Path::new("nonexistent.py"), Position::default());
        assert!(result.is_none());
    }

    #[test]
    fn test_get_document_symbols_no_file() {
        let cache = LspCache::new();
        let result = cache.get_document_symbols(Path::new("nonexistent.py"));
        assert!(result.is_none());
    }

    #[test]
    fn test_get_code_lenses_no_file() {
        let cache = LspCache::new();
        let result = cache.get_code_lenses(Path::new("nonexistent.py"));
        assert!(result.is_none());
    }

    #[test]
    fn test_get_definition_by_name_not_found() {
        let cache = LspCache::new();
        let result = cache.get_definition_by_name("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_symbol_reference() {
        let reference = SymbolReference {
            name: "MyClass".to_string(),
            file_path: PathBuf::from("/test/file.py"),
            line: 10,
            column_start: 5,
            column_end: 12,
            target_name: None,
        };

        assert_eq!(reference.name, "MyClass");
        assert_eq!(reference.line, 10);
    }

    #[test]
    fn test_symbol_definition_lookup() {
        let mut cache = LspCache::new();

        // Manually insert a symbol definition
        let symbol = CachedSymbol {
            name: "TestClass".to_string(),
            kind: SymbolKind::CLASS,
            path: PathBuf::from("/test/module.py"),
            line: 5,
            end_line: 20,
            container: None,
        };
        cache.symbol_definitions.insert("TestClass".to_string(), symbol);

        let location = cache.get_definition_by_name("TestClass");
        assert!(location.is_some());
        let loc = location.unwrap();
        assert_eq!(loc.range.start.line, 5);
    }

    #[test]
    fn test_symbol_definition_case_insensitive_fallback() {
        let mut cache = LspCache::new();

        // Add symbol to the list (not the definitions map)
        let symbol = CachedSymbol {
            name: "MyFunction".to_string(),
            kind: SymbolKind::FUNCTION,
            path: PathBuf::from("/test/funcs.py"),
            line: 15,
            end_line: 25,
            container: None,
        };
        cache.symbols.push(symbol);

        // Should find via case-insensitive search
        let location = cache.get_definition_by_name("myfunction");
        assert!(location.is_some());
        let loc = location.unwrap();
        assert_eq!(loc.range.start.line, 15);
    }

    #[test]
    fn test_get_definition_on_class() {
        let mut cache = LspCache::new();

        let file = FileAnalysis {
            module_name: "test".to_string(),
            path: PathBuf::from("/test/file.py"),
            description: None,
            language: Some(Language::Python),
            dependency_count: 0,
            classes: vec![CachedClass {
                name: "MyClass".to_string(),
                docstring: None,
                line: 5,
                end_line: 20,
                methods: vec![],
            }],
            functions: vec![],
        };

        cache.files.insert("/test/file.py".to_string(), file);

        // Position on the class
        let pos = Position { line: 5, character: 0 };
        let location = cache.get_definition(Path::new("/test/file.py"), pos);
        assert!(location.is_some());
        assert_eq!(location.unwrap().range.start.line, 5);
    }

    #[test]
    fn test_get_definition_on_method() {
        let mut cache = LspCache::new();

        let file = FileAnalysis {
            module_name: "test".to_string(),
            path: PathBuf::from("/test/file.py"),
            description: None,
            language: Some(Language::Python),
            dependency_count: 0,
            classes: vec![CachedClass {
                name: "MyClass".to_string(),
                docstring: None,
                line: 5,
                end_line: 20,
                methods: vec![CachedFunction {
                    name: "my_method".to_string(),
                    docstring: None,
                    line: 10,
                    end_line: 15,
                    signature: "def my_method(self)".to_string(),
                    line_count: 6,
                }],
            }],
            functions: vec![],
        };

        cache.files.insert("/test/file.py".to_string(), file);

        // Position on the method
        let pos = Position { line: 12, character: 0 };
        let location = cache.get_definition(Path::new("/test/file.py"), pos);
        assert!(location.is_some());
        assert_eq!(location.unwrap().range.start.line, 10);
    }

    #[test]
    fn test_get_definition_on_function() {
        let mut cache = LspCache::new();

        let file = FileAnalysis {
            module_name: "test".to_string(),
            path: PathBuf::from("/test/file.py"),
            description: None,
            language: Some(Language::Python),
            dependency_count: 0,
            classes: vec![],
            functions: vec![CachedFunction {
                name: "standalone_func".to_string(),
                docstring: None,
                line: 1,
                end_line: 10,
                signature: "def standalone_func(x, y)".to_string(),
                line_count: 10,
            }],
        };

        cache.files.insert("/test/file.py".to_string(), file);

        let pos = Position { line: 5, character: 0 };
        let location = cache.get_definition(Path::new("/test/file.py"), pos);
        assert!(location.is_some());
        assert_eq!(location.unwrap().range.start.line, 1);
    }
}
