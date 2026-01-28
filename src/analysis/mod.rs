// Analysis module for building and querying code graphs

pub mod graph;
pub mod imports;
pub mod metrics;
pub mod modules;

pub use graph::*;
pub use imports::*;
pub use metrics::*;
pub use modules::*;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::parser::PythonParser;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Result of analyzing a codebase
#[derive(Debug)]
pub struct AnalysisResult {
    /// The code graph with all nodes and edges
    pub graph: CodeGraph,
    /// Detected modules
    pub modules: Vec<Module>,
    /// Project-wide metrics
    pub metrics: ProjectMetrics,
    /// Files that failed to parse (path -> error message)
    pub parse_errors: HashMap<PathBuf, String>,
}

/// Main analyzer that orchestrates the analysis pipeline
pub struct Analyzer {
    config: Config,
    parser: PythonParser,
    verbose: bool,
}

impl Analyzer {
    /// Create a new analyzer with the given configuration
    pub fn new(config: Config) -> Result<Self> {
        let parser = PythonParser::new()?;
        
        Ok(Self {
            config,
            parser,
            verbose: false,
        })
    }
    
    /// Create analyzer with verbose output
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
    
    /// Analyze a codebase at the given path
    pub fn analyze(&mut self, root: &Path) -> Result<AnalysisResult> {
        let root = root.canonicalize().map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Cannot access path: {}", e),
            ))
        })?;
        
        // Step 1: Discover Python files
        let py_files = self.discover_files(&root)?;
        
        if py_files.is_empty() {
            return Err(Error::Analysis("No Python files found".to_string()));
        }
        
        // Step 2: Parse all files and build graph
        let (mut graph, parse_errors) = self.parse_and_build_graph(&py_files, &root)?;
        
        // Step 3: Create import resolver for this project
        let import_resolver = ImportResolver::new(root.clone());
        
        // Step 4: Resolve imports and add edges
        self.resolve_imports(&mut graph, &root, &import_resolver);
        
        // Step 5: Detect modules
        let module_detector = ModuleDetector::new();
        let modules = module_detector.detect(&graph, &root);
        
        // Step 6: Calculate metrics
        let metrics_calculator = MetricsCalculator::new(import_resolver);
        let metrics = metrics_calculator.calculate_project(&graph);
        
        Ok(AnalysisResult {
            graph,
            modules,
            metrics,
            parse_errors,
        })
    }
    
    /// Discover all Python files in the directory
    fn discover_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            // Skip directories
            if path.is_dir() {
                continue;
            }
            
            // Check if it's a Python file
            if let Some(ext) = path.extension() {
                if ext != "py" {
                    continue;
                }
            } else {
                continue;
            }
            
            // Check exclude patterns
            if self.should_exclude(path, root) {
                continue;
            }
            
            files.push(path.to_path_buf());
        }
        
        files.sort();
        Ok(files)
    }
    
    /// Check if a path should be excluded based on config patterns
    fn should_exclude(&self, path: &Path, root: &Path) -> bool {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let relative_str = relative.to_string_lossy();
        
        for pattern in &self.config.analysis.exclude {
            // Simple pattern matching
            if pattern.contains("**") {
                let prefix = pattern.trim_end_matches("/**").trim_end_matches("**");
                if relative_str.starts_with(prefix) {
                    return true;
                }
            } else if pattern.starts_with("*.") {
                let ext = pattern.trim_start_matches("*.");
                if path.extension().map_or(false, |e| e == ext) {
                    return true;
                }
            } else if relative_str.contains(pattern) {
                return true;
            }
        }
        
        // Default exclusions
        let default_excludes = ["__pycache__", ".git", "venv", ".venv", "node_modules", ".tox", ".eggs"];
        for exclude in &default_excludes {
            if relative_str.contains(exclude) {
                return true;
            }
        }
        
        false
    }
    
    /// Parse all files and build the code graph
    fn parse_and_build_graph(
        &mut self,
        files: &[PathBuf],
        root: &Path,
    ) -> Result<(CodeGraph, HashMap<PathBuf, String>)> {
        let mut graph = CodeGraph::new();
        let mut errors = HashMap::new();
        
        let progress = if self.verbose {
            let pb = ProgressBar::new(files.len() as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            Some(pb)
        } else {
            None
        };
        
        for path in files {
            if let Some(ref pb) = progress {
                let msg = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                pb.set_message(msg);
                pb.inc(1);
            }
            
            match self.parser.parse_file(path) {
                Ok(mut parsed_file) => {
                    // Set the module name based on path
                    parsed_file.module_name = self.path_to_module_name(path, root);
                    graph.add_file(&parsed_file);
                }
                Err(e) => {
                    errors.insert(path.clone(), e.to_string());
                }
            }
        }
        
        if let Some(pb) = progress {
            pb.finish_with_message("Parsing complete");
        }
        
        Ok((graph, errors))
    }
    
    /// Convert a file path to a Python module name
    fn path_to_module_name(&self, path: &Path, root: &Path) -> String {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let mut parts: Vec<&str> = relative
            .iter()
            .filter_map(|s| s.to_str())
            .collect();
        
        // Remove .py extension from last part
        if let Some(last) = parts.last_mut() {
            if last.ends_with(".py") {
                *last = last.trim_end_matches(".py");
            }
        }
        
        // Handle __init__.py
        if parts.last() == Some(&"__init__") {
            parts.pop();
        }
        
        parts.join(".")
    }
    
    /// Resolve imports and add edges to the graph
    fn resolve_imports(&self, graph: &mut CodeGraph, root: &Path, resolver: &ImportResolver) {
        // Build a map of module names to file IDs
        let module_map: HashMap<String, FileId> = graph
            .all_files()
            .map(|(id, node)| (node.module_name.clone(), id))
            .collect();
        
        // Collect all edges to add (to avoid borrowing issues)
        let mut edges_to_add = Vec::new();
        
        for (file_id, file_node) in graph.all_files() {
            for import in &file_node.imports {
                let resolved = resolver.resolve(import, &file_node.path);
                
                // Only add edges for local imports
                if resolved.import_type == ImportType::Local {
                    // Try to find the target module by name
                    let target_module = &import.module;
                    
                    // Handle relative imports
                    let resolved_module = if let crate::parser::ImportKind::Relative { level } = &import.kind {
                        // Compute the base module from current file
                        let parts: Vec<&str> = file_node.module_name.split('.').collect();
                        if parts.len() >= *level {
                            let base: Vec<&str> = parts[..parts.len() - level].to_vec();
                            if target_module.is_empty() {
                                base.join(".")
                            } else {
                                format!("{}.{}", base.join("."), target_module)
                            }
                        } else {
                            target_module.clone()
                        }
                    } else {
                        target_module.clone()
                    };
                    
                    if let Some(&target_id) = module_map.get(&resolved_module) {
                        if target_id != file_id {
                            edges_to_add.push(Edge::imports(file_id, target_id));
                        }
                    }
                }
            }
        }
        
        // Add all edges
        for edge in edges_to_add {
            graph.add_edge(edge);
        }
    }
    
    /// Get the file count for reporting
    pub fn file_count(&self, root: &Path) -> Result<usize> {
        self.discover_files(root).map(|f| f.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    fn create_test_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        
        // Create a simple Python project structure
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        
        // Main module
        fs::write(
            src.join("main.py"),
            r#"
"""Main module."""
from .utils import helper

def main():
    """Entry point."""
    helper()
"#,
        )
        .unwrap();
        
        // Utils module
        fs::write(
            src.join("utils.py"),
            r#"
"""Utility functions."""

def helper():
    """A helper function."""
    pass
"#,
        )
        .unwrap();
        
        // Init file
        fs::write(src.join("__init__.py"), "").unwrap();
        
        dir
    }
    
    #[test]
    fn test_analyzer_new() {
        let config = Config::default();
        let analyzer = Analyzer::new(config);
        assert!(analyzer.is_ok());
    }
    
    #[test]
    fn test_discover_files() {
        let dir = create_test_project();
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        
        let files = analyzer.discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 3); // main.py, utils.py, __init__.py
    }
    
    #[test]
    fn test_discover_files_excludes_pycache() {
        let dir = TempDir::new().unwrap();
        
        // Create files
        fs::write(dir.path().join("main.py"), "x = 1").unwrap();
        let pycache = dir.path().join("__pycache__");
        fs::create_dir_all(&pycache).unwrap();
        fs::write(pycache.join("main.cpython-39.pyc"), "").unwrap();
        
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        
        let files = analyzer.discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("main.py"));
    }
    
    #[test]
    fn test_path_to_module_name() {
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        let root = Path::new("/project");
        
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/src/main.py"), root),
            "src.main"
        );
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/src/__init__.py"), root),
            "src"
        );
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/utils.py"), root),
            "utils"
        );
    }
    
    #[test]
    fn test_analyze_simple_project() {
        let dir = create_test_project();
        let config = Config::default();
        let mut analyzer = Analyzer::new(config).unwrap();
        
        let result = analyzer.analyze(dir.path()).unwrap();
        
        // Should have 3 files
        assert_eq!(result.graph.stats().files, 3);
        
        // Should have no parse errors
        assert!(result.parse_errors.is_empty());
        
        // Should have detected at least one module
        assert!(!result.modules.is_empty());
    }
    
    #[test]
    fn test_analyze_empty_directory() {
        let dir = TempDir::new().unwrap();
        let config = Config::default();
        let mut analyzer = Analyzer::new(config).unwrap();
        
        let result = analyzer.analyze(dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No Python files"));
    }
    
    #[test]
    fn test_should_exclude() {
        let mut config = Config::default();
        config.analysis.exclude = vec!["tests/**".to_string(), "*.pyc".to_string()];
        let analyzer = Analyzer::new(config).unwrap();
        let root = Path::new("/project");
        
        assert!(analyzer.should_exclude(Path::new("/project/tests/test_main.py"), root));
        assert!(analyzer.should_exclude(Path::new("/project/__pycache__/main.pyc"), root));
        assert!(!analyzer.should_exclude(Path::new("/project/src/main.py"), root));
    }
    
    #[test]
    fn test_file_count() {
        let dir = create_test_project();
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        
        let count = analyzer.file_count(dir.path()).unwrap();
        assert_eq!(count, 3);
    }
    
    #[test]
    fn test_with_verbose() {
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap().with_verbose(true);
        assert!(analyzer.verbose);
    }
}
