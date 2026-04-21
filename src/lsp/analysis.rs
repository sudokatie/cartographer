//! Analysis bridge for LSP server
//!
//! Wraps cartographer's existing analysis module for use in the LSP server.

use std::path::{Path, PathBuf};

use crate::analysis::{AnalysisResult, Analyzer, Module};
use crate::config::Config;
use crate::error::Result;

/// Bridge between the LSP server and cartographer's analysis module
pub struct AnalysisBridge {
    /// The workspace root path
    workspace_root: Option<PathBuf>,
    /// Cached analysis result
    result: Option<AnalysisResult>,
    /// Configuration for analysis
    config: Config,
}

impl AnalysisBridge {
    /// Create a new AnalysisBridge
    pub fn new() -> Self {
        Self {
            workspace_root: None,
            result: None,
            config: Config::default(),
        }
    }

    /// Create a new AnalysisBridge with custom config
    pub fn with_config(config: Config) -> Self {
        Self {
            workspace_root: None,
            result: None,
            config,
        }
    }

    /// Analyze the workspace at the given root path
    pub fn analyze_workspace(&mut self, root: &Path) -> Result<()> {
        // Find the actual workspace root (go up until we find a project marker)
        let workspace_root = self.find_workspace_root(root);
        self.workspace_root = Some(workspace_root.clone());

        let mut analyzer = Analyzer::new(self.config.clone())?;
        self.result = Some(analyzer.analyze(&workspace_root)?);

        Ok(())
    }

    /// Find the workspace root by looking for project markers
    fn find_workspace_root(&self, path: &Path) -> PathBuf {
        let markers = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "setup.py",
            "go.mod",
            "pom.xml",
            ".git",
        ];

        let mut current = if path.is_file() {
            path.parent().map(|p| p.to_path_buf())
        } else {
            Some(path.to_path_buf())
        };

        while let Some(dir) = current {
            for marker in &markers {
                if dir.join(marker).exists() {
                    return dir;
                }
            }
            current = dir.parent().map(|p| p.to_path_buf());
        }

        // Fall back to the original path
        if path.is_file() {
            path.parent().unwrap_or(path).to_path_buf()
        } else {
            path.to_path_buf()
        }
    }

    /// Get the current analysis result
    pub fn result(&self) -> Option<&AnalysisResult> {
        self.result.as_ref()
    }

    /// Get the workspace root
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }

    /// Get all modules from the analysis
    pub fn modules(&self) -> &[Module] {
        self.result
            .as_ref()
            .map(|r| r.modules.as_slice())
            .unwrap_or(&[])
    }

    /// Check if analysis has been performed
    pub fn has_analysis(&self) -> bool {
        self.result.is_some()
    }

    /// Get file count
    pub fn file_count(&self) -> usize {
        self.result
            .as_ref()
            .map(|r| r.graph.stats().files)
            .unwrap_or(0)
    }

    /// Get class count
    pub fn class_count(&self) -> usize {
        self.result
            .as_ref()
            .map(|r| r.graph.stats().classes)
            .unwrap_or(0)
    }

    /// Get function count
    pub fn function_count(&self) -> usize {
        self.result
            .as_ref()
            .map(|r| r.graph.stats().functions)
            .unwrap_or(0)
    }
}

impl Default for AnalysisBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_analysis_bridge_new() {
        let bridge = AnalysisBridge::new();
        assert!(!bridge.has_analysis());
        assert!(bridge.workspace_root().is_none());
        assert!(bridge.result().is_none());
    }

    #[test]
    fn test_analysis_bridge_default() {
        let bridge = AnalysisBridge::default();
        assert!(!bridge.has_analysis());
    }

    #[test]
    fn test_analysis_bridge_with_config() {
        let config = Config::default();
        let bridge = AnalysisBridge::with_config(config);
        assert!(!bridge.has_analysis());
    }

    #[test]
    fn test_analyze_workspace() {
        let dir = TempDir::new().unwrap();

        // Create a simple Python file
        fs::write(dir.path().join("main.py"), "def hello(): pass").unwrap();

        let mut bridge = AnalysisBridge::new();
        let result = bridge.analyze_workspace(dir.path());

        assert!(result.is_ok());
        assert!(bridge.has_analysis());
        assert!(bridge.workspace_root().is_some());
        assert_eq!(bridge.file_count(), 1);
    }

    #[test]
    fn test_find_workspace_root_with_cargo_toml() {
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

        let bridge = AnalysisBridge::new();
        let root = bridge.find_workspace_root(&src_dir);

        assert_eq!(root, dir.path());
    }

    #[test]
    fn test_modules_empty() {
        let bridge = AnalysisBridge::new();
        assert!(bridge.modules().is_empty());
    }

    #[test]
    fn test_counts_without_analysis() {
        let bridge = AnalysisBridge::new();
        assert_eq!(bridge.file_count(), 0);
        assert_eq!(bridge.class_count(), 0);
        assert_eq!(bridge.function_count(), 0);
    }
}
