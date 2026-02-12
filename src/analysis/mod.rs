// Analysis module for building and querying code graphs

pub mod explain;
pub mod graph;
pub mod imports;
pub mod metrics;
pub mod modules;

pub use explain::*;
pub use graph::*;
pub use imports::*;
pub use metrics::*;
pub use modules::*;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::parser::{GoParser, JavaScriptParser, PythonParser, RustParser};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Rust,
    Go,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "py" => Some(Self::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            "rs" => Some(Self::Rust),
            "go" => Some(Self::Go),
            _ => None,
        }
    }

    /// Check if extension is supported
    pub fn is_supported(ext: &str) -> bool {
        Self::from_extension(ext).is_some()
    }
}

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

/// File counts by language for reporting
#[derive(Debug, Default)]
pub struct LanguageCounts {
    pub python: usize,
    pub javascript: usize,
    pub typescript: usize,
    pub rust: usize,
    pub go: usize,
}

impl LanguageCounts {
    pub fn total(&self) -> usize {
        self.python + self.javascript + self.typescript + self.rust + self.go
    }
}

/// Main analyzer that orchestrates the analysis pipeline
pub struct Analyzer {
    config: Config,
    python_parser: PythonParser,
    js_parser: JavaScriptParser,
    rust_parser: RustParser,
    go_parser: GoParser,
    verbose: bool,
}

impl Analyzer {
    /// Create a new analyzer with the given configuration
    pub fn new(config: Config) -> Result<Self> {
        let python_parser = PythonParser::new()?;
        let js_parser = JavaScriptParser::new()?;
        let rust_parser = RustParser::new()?;
        let go_parser = GoParser::new()?;
        
        Ok(Self {
            config,
            python_parser,
            js_parser,
            rust_parser,
            go_parser,
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
        
        // Step 1: Discover source files (all supported languages)
        let source_files = self.discover_files(&root)?;
        
        if source_files.is_empty() {
            return Err(Error::Analysis("No source files found".to_string()));
        }
        
        // Step 2: Parse all files and build graph
        let (mut graph, parse_errors) = self.parse_and_build_graph(&source_files, &root)?;
        
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
    
    /// Discover all source files in the directory (Python, JS, TS)
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
            
            // Check if it's a supported source file
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e,
                None => continue,
            };
            
            if !Language::is_supported(ext) {
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

    /// Count files by language for reporting
    pub fn file_counts(&self, root: &Path) -> Result<LanguageCounts> {
        let files = self.discover_files(root)?;
        let mut counts = LanguageCounts::default();
        
        for path in files {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                match Language::from_extension(ext) {
                    Some(Language::Python) => counts.python += 1,
                    Some(Language::JavaScript) => counts.javascript += 1,
                    Some(Language::TypeScript) => counts.typescript += 1,
                    Some(Language::Rust) => counts.rust += 1,
                    Some(Language::Go) => counts.go += 1,
                    None => {}
                }
            }
        }
        
        Ok(counts)
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
                if path.extension().is_some_and(|e| e == ext) {
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
            
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let language = Language::from_extension(ext);
            
            let parse_result = match language {
                Some(Language::Python) => self.python_parser.parse_file(path),
                Some(Language::JavaScript) | Some(Language::TypeScript) => {
                    self.js_parser.parse_file(path)
                }
                Some(Language::Rust) => self.rust_parser.parse_file(path),
                Some(Language::Go) => self.go_parser.parse_file(path),
                None => continue,
            };
            
            match parse_result {
                Ok(mut parsed_file) => {
                    // Set the module name based on path and language
                    parsed_file.module_name = self.path_to_module_name(path, root, language);
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
    
    /// Convert a file path to a module name based on language conventions
    fn path_to_module_name(&self, path: &Path, root: &Path, language: Option<Language>) -> String {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let mut parts: Vec<String> = relative
            .iter()
            .filter_map(|s| s.to_str())
            .map(|s| s.to_string())
            .collect();
        
        // Remove extension from last part
        if let Some(last) = parts.last_mut() {
            // Remove any supported extension
            let extensions = [".py", ".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".mts", ".cts", ".rs", ".go"];
            for ext in extensions {
                if last.ends_with(ext) {
                    *last = last.trim_end_matches(ext).to_string();
                    break;
                }
            }
        }
        
        // Handle index files based on language
        match language {
            Some(Language::Python) => {
                // Handle __init__.py
                if parts.last().map(|s| s.as_str()) == Some("__init__") {
                    parts.pop();
                }
            }
            Some(Language::JavaScript) | Some(Language::TypeScript) => {
                // Handle index.js/index.ts
                if parts.last().map(|s| s.as_str()) == Some("index") {
                    parts.pop();
                }
            }
            Some(Language::Rust) => {
                // Handle mod.rs and lib.rs
                let last = parts.last().map(|s| s.as_str());
                if last == Some("mod") || last == Some("lib") {
                    parts.pop();
                }
            }
            Some(Language::Go) => {
                // Go uses package-based naming, no special index handling
            }
            None => {}
        }
        
        // Use :: for Rust, / for JS/TS/Go, . for Python
        match language {
            Some(Language::Rust) => parts.join("::"),
            Some(Language::JavaScript) | Some(Language::TypeScript) | Some(Language::Go) => parts.join("/"),
            _ => parts.join("."),
        }
    }
    
    /// Resolve imports and add edges to the graph
    fn resolve_imports(&self, graph: &mut CodeGraph, _root: &Path, resolver: &ImportResolver) {
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
    fn test_path_to_module_name_python() {
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        let root = Path::new("/project");
        
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/src/main.py"), root, Some(Language::Python)),
            "src.main"
        );
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/src/__init__.py"), root, Some(Language::Python)),
            "src"
        );
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/utils.py"), root, Some(Language::Python)),
            "utils"
        );
    }

    #[test]
    fn test_path_to_module_name_javascript() {
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        let root = Path::new("/project");
        
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/src/main.js"), root, Some(Language::JavaScript)),
            "src/main"
        );
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/src/index.js"), root, Some(Language::JavaScript)),
            "src"
        );
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/utils.ts"), root, Some(Language::TypeScript)),
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
        assert!(result.unwrap_err().to_string().contains("No source files"));
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

    #[test]
    fn test_discover_javascript_files() {
        let dir = TempDir::new().unwrap();
        
        // Create JS/TS files
        fs::write(dir.path().join("app.js"), "const x = 1;").unwrap();
        fs::write(dir.path().join("utils.ts"), "const y: number = 2;").unwrap();
        fs::write(dir.path().join("component.jsx"), "const C = () => <div/>;").unwrap();
        fs::write(dir.path().join("page.tsx"), "const P = (): JSX.Element => <div/>;").unwrap();
        fs::write(dir.path().join("script.py"), "x = 1").unwrap();
        
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        
        let files = analyzer.discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 5); // 4 JS/TS + 1 Python
        
        let counts = analyzer.file_counts(dir.path()).unwrap();
        assert_eq!(counts.python, 1);
        assert_eq!(counts.javascript, 2); // .js and .jsx
        assert_eq!(counts.typescript, 2); // .ts and .tsx
    }

    #[test]
    fn test_analyze_javascript_project() {
        let dir = TempDir::new().unwrap();
        
        // Create a simple JS project
        fs::write(
            dir.path().join("app.js"),
            r#"
import { helper } from './utils.js';

function main() {
    helper();
}
"#,
        ).unwrap();
        
        fs::write(
            dir.path().join("utils.js"),
            r#"
export function helper() {
    console.log('hello');
}
"#,
        ).unwrap();
        
        let config = Config::default();
        let mut analyzer = Analyzer::new(config).unwrap();
        
        let result = analyzer.analyze(dir.path()).unwrap();
        
        // Should have 2 JS files
        assert_eq!(result.graph.stats().files, 2);
        
        // Should have at least 2 functions (main and helper)
        assert!(result.graph.stats().functions >= 2);
    }

    #[test]
    fn test_discover_rust_files() {
        let dir = TempDir::new().unwrap();
        
        // Create Rust files
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("lib.rs"), "pub mod utils;").unwrap();
        fs::write(dir.path().join("utils.rs"), "pub fn helper() {}").unwrap();
        fs::write(dir.path().join("script.py"), "x = 1").unwrap();
        
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        
        let files = analyzer.discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 4); // 3 Rust + 1 Python
        
        let counts = analyzer.file_counts(dir.path()).unwrap();
        assert_eq!(counts.rust, 3);
        assert_eq!(counts.python, 1);
    }

    #[test]
    fn test_analyze_rust_project() {
        let dir = TempDir::new().unwrap();
        
        // Create a simple Rust project
        fs::write(
            dir.path().join("main.rs"),
            r#"
use std::collections::HashMap;

fn main() {
    println!("hello");
}

struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
"#,
        ).unwrap();
        
        let config = Config::default();
        let mut analyzer = Analyzer::new(config).unwrap();
        
        let result = analyzer.analyze(dir.path()).unwrap();
        
        // Should have 1 Rust file
        assert_eq!(result.graph.stats().files, 1);
        
        // Should have main function + Point::new
        assert!(result.graph.stats().functions >= 2);
        
        // Should have Point struct
        assert_eq!(result.graph.stats().classes, 1);
    }

    #[test]
    fn test_path_to_module_name_rust() {
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        let root = Path::new("/project");
        
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/src/main.rs"), root, Some(Language::Rust)),
            "src::main"
        );
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/src/lib.rs"), root, Some(Language::Rust)),
            "src"
        );
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/src/utils/mod.rs"), root, Some(Language::Rust)),
            "src::utils"
        );
    }

    #[test]
    fn test_discover_go_files() {
        let dir = TempDir::new().unwrap();
        
        // Create Go files
        fs::write(dir.path().join("main.go"), "package main").unwrap();
        fs::write(dir.path().join("utils.go"), "package main").unwrap();
        fs::write(dir.path().join("utils_test.go"), "package main").unwrap();
        fs::write(dir.path().join("script.py"), "x = 1").unwrap();
        
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        
        let files = analyzer.discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 4); // 3 Go + 1 Python
        
        let counts = analyzer.file_counts(dir.path()).unwrap();
        assert_eq!(counts.go, 3);
        assert_eq!(counts.python, 1);
    }

    #[test]
    fn test_analyze_go_project() {
        let dir = TempDir::new().unwrap();
        
        // Create a simple Go project
        fs::write(
            dir.path().join("main.go"),
            r#"
package main

import "fmt"

func main() {
    fmt.Println("hello")
}

type Point struct {
    X float64
    Y float64
}

func (p *Point) Distance(other Point) float64 {
    return 0.0
}
"#,
        ).unwrap();
        
        let config = Config::default();
        let mut analyzer = Analyzer::new(config).unwrap();
        
        let result = analyzer.analyze(dir.path()).unwrap();
        
        // Should have 1 Go file
        assert_eq!(result.graph.stats().files, 1);
        
        // Should have main + Point.Distance
        assert!(result.graph.stats().functions >= 2);
        
        // Should have Point struct
        assert_eq!(result.graph.stats().classes, 1);
    }

    #[test]
    fn test_path_to_module_name_go() {
        let config = Config::default();
        let analyzer = Analyzer::new(config).unwrap();
        let root = Path::new("/project");
        
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/cmd/main.go"), root, Some(Language::Go)),
            "cmd/main"
        );
        assert_eq!(
            analyzer.path_to_module_name(Path::new("/project/pkg/utils/utils.go"), root, Some(Language::Go)),
            "pkg/utils/utils"
        );
    }
}
