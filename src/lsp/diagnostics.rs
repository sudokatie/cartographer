//! Diagnostics provider for the LSP server
//!
//! Generates diagnostics from analysis results:
//! - Circular dependencies (Error)
//! - Missing module documentation (Hint)
//! - High complexity modules (Warning)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::*;

use crate::analysis::graph::{CodeGraph, FileId};
use crate::analysis::metrics::FileMetrics;
use crate::lsp::analysis::AnalysisBridge;

/// Threshold for high complexity warning
const HIGH_COMPLEXITY_THRESHOLD: usize = 20;

/// Threshold for high function count warning
const HIGH_FUNCTION_COUNT_THRESHOLD: usize = 15;

/// Diagnostics provider for generating LSP diagnostics
pub struct DiagnosticsProvider;

impl DiagnosticsProvider {
    /// Generate all diagnostics from analysis results
    pub fn generate_diagnostics(
        analysis: &AnalysisBridge,
        open_files: &[PathBuf],
    ) -> HashMap<Url, Vec<Diagnostic>> {
        let mut diagnostics: HashMap<Url, Vec<Diagnostic>> = HashMap::new();

        // Initialize empty diagnostics for all open files
        for path in open_files {
            if let Ok(uri) = Url::from_file_path(path) {
                diagnostics.entry(uri).or_default();
            }
        }

        let Some(result) = analysis.result() else {
            return diagnostics;
        };

        // Calculate file metrics for complexity checks
        let file_metrics = Self::calculate_file_metrics(&result.graph);

        // Generate circular dependency diagnostics
        Self::add_circular_dependency_diagnostics(&result.graph, open_files, &mut diagnostics);

        // Generate missing documentation diagnostics
        Self::add_missing_docs_diagnostics(&result.graph, open_files, &mut diagnostics);

        // Generate high complexity diagnostics
        Self::add_complexity_diagnostics(
            &result.graph,
            &file_metrics,
            open_files,
            &mut diagnostics,
        );

        diagnostics
    }

    /// Calculate file metrics for all files in the graph
    fn calculate_file_metrics(graph: &CodeGraph) -> HashMap<FileId, FileMetrics> {
        let mut metrics = HashMap::new();
        for (file_id, _) in graph.all_files() {
            metrics.insert(file_id, FileMetrics::from_graph(graph, file_id));
        }
        metrics
    }

    /// Add diagnostics for circular dependencies
    fn add_circular_dependency_diagnostics(
        graph: &CodeGraph,
        open_files: &[PathBuf],
        diagnostics: &mut HashMap<Url, Vec<Diagnostic>>,
    ) {
        let cycles = graph.detect_circular_dependencies();

        for cycle in cycles {
            // Get file paths for the cycle
            let cycle_paths: Vec<PathBuf> = cycle
                .iter()
                .filter_map(|&file_id| graph.get_file(file_id).map(|f| f.path.clone()))
                .collect();

            // Create a description of the cycle
            let cycle_names: Vec<String> = cycle_paths
                .iter()
                .map(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                })
                .collect();
            let cycle_description = cycle_names.join(" -> ");

            // Add diagnostic to each file in the cycle that is open
            for path in &cycle_paths {
                if !Self::is_file_open(path, open_files) {
                    continue;
                }

                if let Ok(uri) = Url::from_file_path(path) {
                    let diagnostic = Diagnostic {
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: 0,
                            },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: Some(NumberOrString::String("circular-dependency".to_string())),
                        code_description: None,
                        source: Some("cartographer".to_string()),
                        message: format!(
                            "Circular dependency detected: {}",
                            cycle_description
                        ),
                        related_information: Some(
                            cycle_paths
                                .iter()
                                .filter(|p| *p != path)
                                .filter_map(|p| {
                                    let uri = Url::from_file_path(p).ok()?;
                                    Some(DiagnosticRelatedInformation {
                                        location: Location {
                                            uri,
                                            range: Range::default(),
                                        },
                                        message: "Part of circular dependency".to_string(),
                                    })
                                })
                                .collect(),
                        ),
                        tags: None,
                        data: None,
                    };

                    diagnostics.entry(uri).or_default().push(diagnostic);
                }
            }
        }
    }

    /// Add diagnostics for missing module documentation
    fn add_missing_docs_diagnostics(
        graph: &CodeGraph,
        open_files: &[PathBuf],
        diagnostics: &mut HashMap<Url, Vec<Diagnostic>>,
    ) {
        for (_, file_node) in graph.all_files() {
            if !Self::is_file_open(&file_node.path, open_files) {
                continue;
            }

            // Check if file has a docstring
            if file_node.docstring.is_none() || file_node.docstring.as_ref().map(|d| d.trim().is_empty()).unwrap_or(false) {
                if let Ok(uri) = Url::from_file_path(&file_node.path) {
                    let diagnostic = Diagnostic {
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: 0,
                            },
                        },
                        severity: Some(DiagnosticSeverity::HINT),
                        code: Some(NumberOrString::String("missing-module-docs".to_string())),
                        code_description: None,
                        source: Some("cartographer".to_string()),
                        message: "Module is missing documentation".to_string(),
                        related_information: None,
                        tags: None,
                        data: None,
                    };

                    diagnostics.entry(uri).or_default().push(diagnostic);
                }
            }

            // Check classes for missing docstrings
            for &class_id in &file_node.classes {
                if let Some(class) = graph.get_class(class_id) {
                    if class.docstring.is_none() || class.docstring.as_ref().map(|d| d.trim().is_empty()).unwrap_or(false) {
                        if let Ok(uri) = Url::from_file_path(&file_node.path) {
                            let diagnostic = Diagnostic {
                                range: Range {
                                    start: Position {
                                        line: class.line_start.saturating_sub(1) as u32,
                                        character: 0,
                                    },
                                    end: Position {
                                        line: class.line_start.saturating_sub(1) as u32,
                                        character: 0,
                                    },
                                },
                                severity: Some(DiagnosticSeverity::HINT),
                                code: Some(NumberOrString::String("missing-class-docs".to_string())),
                                code_description: None,
                                source: Some("cartographer".to_string()),
                                message: format!("Class '{}' is missing documentation", class.name),
                                related_information: None,
                                tags: None,
                                data: None,
                            };

                            diagnostics.entry(uri).or_default().push(diagnostic);
                        }
                    }
                }
            }
        }
    }

    /// Add diagnostics for high complexity
    fn add_complexity_diagnostics(
        graph: &CodeGraph,
        file_metrics: &HashMap<FileId, FileMetrics>,
        open_files: &[PathBuf],
        diagnostics: &mut HashMap<Url, Vec<Diagnostic>>,
    ) {
        for (file_id, file_node) in graph.all_files() {
            if !Self::is_file_open(&file_node.path, open_files) {
                continue;
            }

            let Some(metrics) = file_metrics.get(&file_id) else {
                continue;
            };

            // Check overall file complexity
            if metrics.complexity > HIGH_COMPLEXITY_THRESHOLD {
                if let Ok(uri) = Url::from_file_path(&file_node.path) {
                    let diagnostic = Diagnostic {
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: 0,
                            },
                        },
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(NumberOrString::String("high-complexity".to_string())),
                        code_description: None,
                        source: Some("cartographer".to_string()),
                        message: format!(
                            "High complexity detected (complexity: {}, threshold: {}). Consider splitting this module.",
                            metrics.complexity, HIGH_COMPLEXITY_THRESHOLD
                        ),
                        related_information: None,
                        tags: None,
                        data: None,
                    };

                    diagnostics.entry(uri).or_default().push(diagnostic);
                }
            }

            // Check function count
            if metrics.function_count > HIGH_FUNCTION_COUNT_THRESHOLD {
                if let Ok(uri) = Url::from_file_path(&file_node.path) {
                    let diagnostic = Diagnostic {
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: 0,
                            },
                        },
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(NumberOrString::String("too-many-functions".to_string())),
                        code_description: None,
                        source: Some("cartographer".to_string()),
                        message: format!(
                            "Too many functions in module ({} functions, threshold: {}). Consider splitting this module.",
                            metrics.function_count, HIGH_FUNCTION_COUNT_THRESHOLD
                        ),
                        related_information: None,
                        tags: None,
                        data: None,
                    };

                    diagnostics.entry(uri).or_default().push(diagnostic);
                }
            }
        }
    }

    /// Check if a file is in the open files list
    fn is_file_open(path: &Path, open_files: &[PathBuf]) -> bool {
        open_files.iter().any(|open| open == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::graph::Edge;
    use crate::parser::{Class, Function, ParsedFile};
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[allow(dead_code)]
    fn make_parsed_file(name: &str) -> ParsedFile {
        let path = PathBuf::from(format!("{}.py", name));
        ParsedFile::new(path, name.to_string())
    }

    #[test]
    fn test_generate_diagnostics_empty() {
        let bridge = AnalysisBridge::new();
        let open_files: Vec<PathBuf> = vec![];
        let diagnostics = DiagnosticsProvider::generate_diagnostics(&bridge, &open_files);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_circular_dependency_diagnostic() {
        let dir = TempDir::new().unwrap();
        let path_a = dir.path().join("a.py");
        let path_b = dir.path().join("b.py");
        let path_c = dir.path().join("c.py");

        std::fs::write(&path_a, "import b").unwrap();
        std::fs::write(&path_b, "import c").unwrap();
        std::fs::write(&path_c, "import a").unwrap();

        let mut graph = CodeGraph::new();
        let mut file_a = ParsedFile::new(path_a.clone(), "a".to_string());
        let mut file_b = ParsedFile::new(path_b.clone(), "b".to_string());
        let mut file_c = ParsedFile::new(path_c.clone(), "c".to_string());

        file_a.docstring = Some("Module A".to_string());
        file_b.docstring = Some("Module B".to_string());
        file_c.docstring = Some("Module C".to_string());

        let id_a = graph.add_file(&file_a);
        let id_b = graph.add_file(&file_b);
        let id_c = graph.add_file(&file_c);

        // Create cycle: a -> b -> c -> a
        graph.add_edge(Edge::imports(id_a, id_b));
        graph.add_edge(Edge::imports(id_b, id_c));
        graph.add_edge(Edge::imports(id_c, id_a));

        let cycles = graph.detect_circular_dependencies();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn test_missing_docs_diagnostic() {
        let mut graph = CodeGraph::new();
        let path = PathBuf::from("/test/missing_docs.py");
        let file = ParsedFile::new(path.clone(), "missing_docs".to_string());
        graph.add_file(&file);

        let open_files = vec![path.clone()];
        let mut diagnostics: HashMap<Url, Vec<Diagnostic>> = HashMap::new();

        DiagnosticsProvider::add_missing_docs_diagnostics(&graph, &open_files, &mut diagnostics);

        let uri = Url::from_file_path(&path).unwrap();
        assert!(diagnostics.contains_key(&uri));
        let file_diags = diagnostics.get(&uri).unwrap();
        assert!(!file_diags.is_empty());
        assert_eq!(
            file_diags[0].code,
            Some(NumberOrString::String("missing-module-docs".to_string()))
        );
    }

    #[test]
    fn test_high_complexity_diagnostic() {
        let mut graph = CodeGraph::new();
        let path = PathBuf::from("/test/complex.py");
        let mut file = ParsedFile::new(path.clone(), "complex".to_string());
        file.docstring = Some("Complex module".to_string());

        // Add many functions to trigger high complexity
        for i in 0..25 {
            file.functions.push(Function::new(&format!("func_{}", i), i + 1));
        }

        let file_id = graph.add_file(&file);
        let file_metrics = {
            let mut m = HashMap::new();
            m.insert(file_id, FileMetrics::from_graph(&graph, file_id));
            m
        };

        let open_files = vec![path.clone()];
        let mut diagnostics: HashMap<Url, Vec<Diagnostic>> = HashMap::new();

        DiagnosticsProvider::add_complexity_diagnostics(
            &graph,
            &file_metrics,
            &open_files,
            &mut diagnostics,
        );

        let uri = Url::from_file_path(&path).unwrap();
        assert!(diagnostics.contains_key(&uri));
        let file_diags = diagnostics.get(&uri).unwrap();
        assert!(!file_diags.is_empty());

        // Should have both high-complexity and too-many-functions diagnostics
        let codes: Vec<_> = file_diags
            .iter()
            .filter_map(|d| d.code.as_ref())
            .collect();
        assert!(codes.iter().any(|c| matches!(c, NumberOrString::String(s) if s == "high-complexity")));
        assert!(codes.iter().any(|c| matches!(c, NumberOrString::String(s) if s == "too-many-functions")));
    }

    #[test]
    fn test_is_file_open() {
        let open_files = vec![
            PathBuf::from("/test/a.py"),
            PathBuf::from("/test/b.py"),
        ];

        assert!(DiagnosticsProvider::is_file_open(
            Path::new("/test/a.py"),
            &open_files
        ));
        assert!(!DiagnosticsProvider::is_file_open(
            Path::new("/test/c.py"),
            &open_files
        ));
    }

    #[test]
    fn test_missing_class_docs_diagnostic() {
        let mut graph = CodeGraph::new();
        let path = PathBuf::from("/test/class_no_docs.py");
        let mut file = ParsedFile::new(path.clone(), "class_no_docs".to_string());
        file.docstring = Some("Module with class".to_string());

        // Add a class without docstring
        let mut class = Class::new("MyClass", 5);
        class.line_end = 20;
        file.classes.push(class);

        graph.add_file(&file);

        let open_files = vec![path.clone()];
        let mut diagnostics: HashMap<Url, Vec<Diagnostic>> = HashMap::new();

        DiagnosticsProvider::add_missing_docs_diagnostics(&graph, &open_files, &mut diagnostics);

        let uri = Url::from_file_path(&path).unwrap();
        assert!(diagnostics.contains_key(&uri));
        let file_diags = diagnostics.get(&uri).unwrap();
        assert!(file_diags.iter().any(|d| {
            d.code == Some(NumberOrString::String("missing-class-docs".to_string()))
        }));
    }
}
