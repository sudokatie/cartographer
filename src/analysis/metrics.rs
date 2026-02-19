// Metrics calculation for code analysis
//
// Calculates per-file and per-module metrics including:
// - Lines of code (excluding blanks/comments)
// - Number of classes, functions
// - Import counts by type
// - Cyclomatic complexity (basic)
// - Public/private ratio

use crate::analysis::graph::{CodeGraph, FileId};
use crate::analysis::imports::{ImportResolver, ImportType, ResolvedImport};
use crate::analysis::modules::Module;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metrics for a single file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileMetrics {
    /// Total lines in file
    pub total_lines: usize,
    /// Lines of code (non-blank, non-comment)
    pub code_lines: usize,
    /// Comment lines
    pub comment_lines: usize,
    /// Blank lines
    pub blank_lines: usize,
    /// Number of classes
    pub class_count: usize,
    /// Number of functions (including methods)
    pub function_count: usize,
    /// Number of top-level functions (not methods)
    pub top_level_function_count: usize,
    /// Number of methods (functions inside classes)
    pub method_count: usize,
    /// Number of constants (ALL_CAPS)
    pub constant_count: usize,
    /// Import counts by type
    pub imports: ImportMetrics,
    /// Average function length (lines)
    pub avg_function_length: f64,
    /// Maximum function length (lines)
    pub max_function_length: usize,
    /// Public/private ratio (0.0 to 1.0)
    pub public_ratio: f64,
    /// Basic cyclomatic complexity estimate
    pub complexity: usize,
}

impl FileMetrics {
    /// Calculate metrics from a code graph file
    pub fn from_graph(graph: &CodeGraph, file_id: FileId) -> Self {
        let file = match graph.get_file(file_id) {
            Some(f) => f,
            None => return Self::default(),
        };

        let total_lines = file.total_lines;
        let code_lines = file.code_lines;
        let comment_lines = file.comment_lines;
        let blank_lines = total_lines.saturating_sub(code_lines + comment_lines);

        let class_count = file.classes.len();
        let constant_count = file.constants.len();

        // Count functions and methods
        let top_level_function_count = file.functions.len();
        let mut method_count = 0;
        let mut function_lengths: Vec<usize> = Vec::new();
        let mut public_count = 0;
        let mut private_count = 0;

        // Get function lengths from top-level functions
        for &func_id in &file.functions {
            if let Some(func) = graph.get_function(func_id) {
                let length = func.line_end.saturating_sub(func.line_start) + 1;
                function_lengths.push(length);

                if func.name.starts_with('_') && !func.name.starts_with("__") {
                    private_count += 1;
                } else {
                    public_count += 1;
                }
            }
        }

        // Count methods from classes
        for &class_id in &file.classes {
            if let Some(class) = graph.get_class(class_id) {
                method_count += class.methods.len();

                for &method_id in &class.methods {
                    if let Some(method) = graph.get_function(method_id) {
                        let length = method.line_end.saturating_sub(method.line_start) + 1;
                        function_lengths.push(length);

                        if method.name.starts_with('_') && !method.name.starts_with("__") {
                            private_count += 1;
                        } else {
                            public_count += 1;
                        }
                    }
                }
            }
        }

        let function_count = top_level_function_count + method_count;
        let avg_function_length = if function_lengths.is_empty() {
            0.0
        } else {
            function_lengths.iter().sum::<usize>() as f64 / function_lengths.len() as f64
        };
        let max_function_length = function_lengths.iter().copied().max().unwrap_or(0);

        let total_entities = public_count + private_count;
        let public_ratio = if total_entities > 0 {
            public_count as f64 / total_entities as f64
        } else {
            1.0 // No functions = consider all public
        };

        // Basic complexity estimate (1 per function + 1 per class)
        let complexity = function_count + class_count;

        Self {
            total_lines,
            code_lines,
            comment_lines,
            blank_lines,
            class_count,
            function_count,
            top_level_function_count,
            method_count,
            constant_count,
            imports: ImportMetrics::default(),
            avg_function_length,
            max_function_length,
            public_ratio,
            complexity,
        }
    }

    /// Add import metrics from resolved imports
    pub fn with_imports(mut self, resolved: &[ResolvedImport]) -> Self {
        self.imports = ImportMetrics::from_resolved(resolved);
        self
    }
}

/// Import-related metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportMetrics {
    /// Total number of imports
    pub total: usize,
    /// Standard library imports
    pub stdlib: usize,
    /// Third-party imports
    pub third_party: usize,
    /// Local project imports
    pub local: usize,
    /// Unknown/unresolved imports
    pub unknown: usize,
}

impl ImportMetrics {
    /// Calculate import metrics from resolved imports
    pub fn from_resolved(resolved: &[ResolvedImport]) -> Self {
        let mut stdlib = 0;
        let mut third_party = 0;
        let mut local = 0;
        let mut unknown = 0;

        for import in resolved {
            match import.import_type {
                ImportType::Stdlib => stdlib += 1,
                ImportType::ThirdParty => third_party += 1,
                ImportType::Local => local += 1,
                ImportType::Unknown => unknown += 1,
            }
        }

        Self {
            total: resolved.len(),
            stdlib,
            third_party,
            local,
            unknown,
        }
    }
}

/// Metrics for a module (group of files)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModuleMetrics {
    /// Module name
    pub name: String,
    /// Number of files in module
    pub file_count: usize,
    /// Aggregated file metrics
    pub totals: AggregatedMetrics,
    /// Averages per file
    pub averages: AggregatedMetrics,
}

impl ModuleMetrics {
    /// Calculate metrics for a module
    pub fn from_module(module: &Module, file_metrics: &HashMap<FileId, FileMetrics>) -> Self {
        let file_count = module.files.len();
        let mut totals = AggregatedMetrics::default();

        for &file_id in &module.files {
            if let Some(metrics) = file_metrics.get(&file_id) {
                totals.total_lines += metrics.total_lines;
                totals.code_lines += metrics.code_lines;
                totals.comment_lines += metrics.comment_lines;
                totals.class_count += metrics.class_count;
                totals.function_count += metrics.function_count;
                totals.import_count += metrics.imports.total;
                totals.complexity += metrics.complexity;
            }
        }

        let averages = if file_count > 0 {
            AggregatedMetrics {
                total_lines: totals.total_lines / file_count,
                code_lines: totals.code_lines / file_count,
                comment_lines: totals.comment_lines / file_count,
                class_count: totals.class_count / file_count,
                function_count: totals.function_count / file_count,
                import_count: totals.import_count / file_count,
                complexity: totals.complexity / file_count,
            }
        } else {
            AggregatedMetrics::default()
        };

        Self {
            name: module.name.clone(),
            file_count,
            totals,
            averages,
        }
    }
}

/// Aggregated metrics (used for totals and averages)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub total_lines: usize,
    pub code_lines: usize,
    pub comment_lines: usize,
    pub class_count: usize,
    pub function_count: usize,
    pub import_count: usize,
    pub complexity: usize,
}

/// Project-wide metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMetrics {
    /// Total files analyzed
    pub file_count: usize,
    /// Total lines of code
    pub total_lines: usize,
    /// Code lines (non-blank, non-comment)
    pub code_lines: usize,
    /// Comment lines
    pub comment_lines: usize,
    /// Total classes
    pub class_count: usize,
    /// Total functions
    pub function_count: usize,
    /// Total imports
    pub import_count: usize,
    /// Import breakdown
    pub imports: ImportMetrics,
    /// Average file size (lines)
    pub avg_file_size: f64,
    /// Average function length
    pub avg_function_length: f64,
    /// Maximum file size
    pub max_file_size: usize,
    /// Overall complexity
    pub total_complexity: usize,
    /// Comment ratio (comment lines / code lines)
    pub comment_ratio: f64,
}

impl ProjectMetrics {
    /// Calculate project-wide metrics from file metrics
    pub fn from_files(file_metrics: &HashMap<FileId, FileMetrics>) -> Self {
        let file_count = file_metrics.len();
        let mut metrics = Self {
            file_count,
            ..Default::default()
        };

        let mut function_lengths: Vec<f64> = Vec::new();

        for file in file_metrics.values() {
            metrics.total_lines += file.total_lines;
            metrics.code_lines += file.code_lines;
            metrics.comment_lines += file.comment_lines;
            metrics.class_count += file.class_count;
            metrics.function_count += file.function_count;
            metrics.import_count += file.imports.total;
            metrics.imports.stdlib += file.imports.stdlib;
            metrics.imports.third_party += file.imports.third_party;
            metrics.imports.local += file.imports.local;
            metrics.imports.unknown += file.imports.unknown;
            metrics.total_complexity += file.complexity;

            if file.total_lines > metrics.max_file_size {
                metrics.max_file_size = file.total_lines;
            }

            if file.function_count > 0 {
                function_lengths.push(file.avg_function_length);
            }
        }

        metrics.imports.total = metrics.import_count;

        metrics.avg_file_size = if file_count > 0 {
            metrics.total_lines as f64 / file_count as f64
        } else {
            0.0
        };

        metrics.avg_function_length = if !function_lengths.is_empty() {
            function_lengths.iter().sum::<f64>() / function_lengths.len() as f64
        } else {
            0.0
        };

        metrics.comment_ratio = if metrics.code_lines > 0 {
            metrics.comment_lines as f64 / metrics.code_lines as f64
        } else {
            0.0
        };

        metrics
    }
}

/// Metrics calculator
pub struct MetricsCalculator {
    resolver: ImportResolver,
}

impl MetricsCalculator {
    /// Create a new metrics calculator
    pub fn new(resolver: ImportResolver) -> Self {
        Self { resolver }
    }

    /// Calculate metrics for all files in a graph
    pub fn calculate_all(
        &self,
        graph: &CodeGraph,
    ) -> HashMap<FileId, FileMetrics> {
        let mut results = HashMap::new();

        for (file_id, file_node) in graph.all_files() {
            let mut metrics = FileMetrics::from_graph(graph, file_id);

            // Resolve imports and add import metrics
            let resolved = self.resolver.resolve_all(&file_node.imports, &file_node.path);
            metrics = metrics.with_imports(&resolved);

            results.insert(file_id, metrics);
        }

        results
    }

    /// Calculate project-wide metrics
    pub fn calculate_project(&self, graph: &CodeGraph) -> ProjectMetrics {
        let file_metrics = self.calculate_all(graph);
        ProjectMetrics::from_files(&file_metrics)
    }

    /// Calculate metrics for modules
    pub fn calculate_modules(
        &self,
        modules: &[Module],
        file_metrics: &HashMap<FileId, FileMetrics>,
    ) -> Vec<ModuleMetrics> {
        modules
            .iter()
            .map(|m| ModuleMetrics::from_module(m, file_metrics))
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::parser::{Function, Import, ParsedFile};
    use std::path::PathBuf;

    fn make_test_file() -> ParsedFile {
        let mut file = ParsedFile::new(PathBuf::from("test.py"), "test".to_string());
        file.total_lines = 100;
        file.code_lines = 80;
        file.comment_lines = 10;

        // Add a function
        let func = Function::new("my_func", 10);
        file.functions.push(func);

        // Add an import
        file.imports.push(Import::simple("os", 1));

        file
    }

    #[test]
    fn test_file_metrics_basic() {
        let mut graph = CodeGraph::new();
        let file = make_test_file();
        let file_id = graph.add_file(&file);

        let metrics = FileMetrics::from_graph(&graph, file_id);

        assert_eq!(metrics.total_lines, 100);
        assert_eq!(metrics.code_lines, 80);
        assert_eq!(metrics.comment_lines, 10);
        assert_eq!(metrics.blank_lines, 10);
        assert_eq!(metrics.top_level_function_count, 1);
    }

    #[test]
    fn test_import_metrics() {
        let resolved = vec![
            ResolvedImport {
                import: Import::simple("os", 1),
                import_type: ImportType::Stdlib,
                resolved_path: None,
                resolved_module: "os".to_string(),
            },
            ResolvedImport {
                import: Import::simple("requests", 2),
                import_type: ImportType::ThirdParty,
                resolved_path: None,
                resolved_module: "requests".to_string(),
            },
            ResolvedImport {
                import: Import::simple("mymodule", 3),
                import_type: ImportType::Local,
                resolved_path: Some(PathBuf::from("mymodule.py")),
                resolved_module: "mymodule".to_string(),
            },
        ];

        let metrics = ImportMetrics::from_resolved(&resolved);

        assert_eq!(metrics.total, 3);
        assert_eq!(metrics.stdlib, 1);
        assert_eq!(metrics.third_party, 1);
        assert_eq!(metrics.local, 1);
    }

    #[test]
    fn test_project_metrics() {
        let mut file_metrics = HashMap::new();

        let mut m1 = FileMetrics::default();
        m1.total_lines = 100;
        m1.code_lines = 80;
        m1.function_count = 5;
        m1.imports.total = 3;
        m1.imports.stdlib = 2;
        m1.imports.local = 1;
        file_metrics.insert(FileId(0), m1);

        let mut m2 = FileMetrics::default();
        m2.total_lines = 50;
        m2.code_lines = 40;
        m2.function_count = 2;
        m2.imports.total = 2;
        m2.imports.third_party = 2;
        file_metrics.insert(FileId(1), m2);

        let project = ProjectMetrics::from_files(&file_metrics);

        assert_eq!(project.file_count, 2);
        assert_eq!(project.total_lines, 150);
        assert_eq!(project.code_lines, 120);
        assert_eq!(project.function_count, 7);
        assert_eq!(project.import_count, 5);
        assert_eq!(project.imports.stdlib, 2);
        assert_eq!(project.imports.third_party, 2);
        assert_eq!(project.imports.local, 1);
        assert_eq!(project.max_file_size, 100);
    }

    #[test]
    fn test_module_metrics() {
        let module = Module {
            name: "test".to_string(),
            path: PathBuf::from("test"),
            files: vec![FileId(0), FileId(1)],
            is_package: true,
            module_type: crate::analysis::modules::ModuleType::Generic,
            outgoing_imports: 0,
            incoming_imports: 0,
        };

        let mut file_metrics = HashMap::new();

        let mut m1 = FileMetrics::default();
        m1.code_lines = 100;
        m1.function_count = 5;
        file_metrics.insert(FileId(0), m1);

        let mut m2 = FileMetrics::default();
        m2.code_lines = 50;
        m2.function_count = 3;
        file_metrics.insert(FileId(1), m2);

        let module_metrics = ModuleMetrics::from_module(&module, &file_metrics);

        assert_eq!(module_metrics.file_count, 2);
        assert_eq!(module_metrics.totals.code_lines, 150);
        assert_eq!(module_metrics.totals.function_count, 8);
        assert_eq!(module_metrics.averages.code_lines, 75);
        assert_eq!(module_metrics.averages.function_count, 4);
    }

    #[test]
    fn test_public_ratio() {
        let mut graph = CodeGraph::new();
        let mut file = ParsedFile::new(PathBuf::from("test.py"), "test".to_string());

        // Add public function
        file.functions.push(Function::new("public_func", 1));
        // Add private function
        file.functions.push(Function::new("_private_func", 5));
        // Add special method (counts as public)
        file.functions.push(Function::new("__init__", 10));

        let file_id = graph.add_file(&file);
        let metrics = FileMetrics::from_graph(&graph, file_id);

        // 2 public (public_func, __init__), 1 private (_private_func)
        // public_ratio = 2/3 = 0.666...
        assert!((metrics.public_ratio - 0.666).abs() < 0.01);
    }
}
