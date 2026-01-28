// Module detection and grouping
//
// Groups files into logical modules based on directory structure
// and common naming patterns.

use crate::analysis::graph::{CodeGraph, FileId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A detected module (logical grouping of files)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// Module name (e.g., "models", "utils", "api.handlers")
    pub name: String,
    /// Module path relative to project root
    pub path: PathBuf,
    /// IDs of files in this module
    pub files: Vec<FileId>,
    /// Is this a Python package (has __init__.py)?
    pub is_package: bool,
    /// Detected module type based on naming patterns
    pub module_type: ModuleType,
    /// Number of imports from other modules (coupling metric)
    pub outgoing_imports: usize,
    /// Number of times imported by other modules
    pub incoming_imports: usize,
}

impl Module {
    /// Calculate import density (coupling metric)
    pub fn coupling_score(&self) -> f64 {
        if self.files.is_empty() {
            return 0.0;
        }
        (self.outgoing_imports + self.incoming_imports) as f64 / self.files.len() as f64
    }
}

/// Type of module based on common naming patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ModuleType {
    /// Data models (models/, schemas/, entities/)
    Models,
    /// View/presentation layer (views/, templates/, pages/)
    Views,
    /// Service/business logic (services/, handlers/, controllers/)
    Services,
    /// Utility functions (utils/, helpers/, lib/)
    Utils,
    /// API endpoints (api/, routes/, endpoints/)
    Api,
    /// Tests (tests/, test/, testing/)
    Tests,
    /// Configuration (config/, settings/, conf/)
    Config,
    /// Core/main module (core/, main/, app/)
    Core,
    /// Generic/unknown type
    #[default]
    Generic,
}

impl ModuleType {
    /// Detect module type from directory name
    pub fn from_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        match lower.as_str() {
            "models" | "schemas" | "entities" | "domain" => ModuleType::Models,
            "views" | "templates" | "pages" | "ui" => ModuleType::Views,
            "services" | "handlers" | "controllers" | "actions" => ModuleType::Services,
            "utils" | "helpers" | "lib" | "common" | "shared" => ModuleType::Utils,
            "api" | "routes" | "endpoints" | "resources" => ModuleType::Api,
            "tests" | "test" | "testing" | "specs" => ModuleType::Tests,
            "config" | "settings" | "conf" | "configuration" => ModuleType::Config,
            "core" | "main" | "app" | "application" => ModuleType::Core,
            _ => ModuleType::Generic,
        }
    }
}

/// Detects and groups files into modules
pub struct ModuleDetector {
    /// Minimum files to consider a directory a module
    min_files: usize,
}

impl ModuleDetector {
    /// Create a new module detector
    pub fn new() -> Self {
        Self { min_files: 1 }
    }

    /// Set minimum files required to form a module
    pub fn with_min_files(mut self, min: usize) -> Self {
        self.min_files = min;
        self
    }

    /// Detect modules from a code graph
    pub fn detect(&self, graph: &CodeGraph, project_root: &Path) -> Vec<Module> {
        // Group files by their parent directory
        let mut dir_files: HashMap<PathBuf, Vec<FileId>> = HashMap::new();
        let mut dir_has_init: HashMap<PathBuf, bool> = HashMap::new();

        for (file_id, file_node) in graph.all_files() {
            // Get the directory containing this file
            let file_path = if file_node.path.is_absolute() {
                file_node.path.clone()
            } else {
                project_root.join(&file_node.path)
            };

            if let Some(parent) = file_path.parent() {
                let relative_parent = parent
                    .strip_prefix(project_root)
                    .unwrap_or(parent)
                    .to_path_buf();

                dir_files
                    .entry(relative_parent.clone())
                    .or_default()
                    .push(file_id);

                // Check if this is an __init__.py
                if file_node.path.file_name().map(|n| n == "__init__.py").unwrap_or(false) {
                    dir_has_init.insert(relative_parent, true);
                }
            }
        }

        // Convert directories to modules
        let mut modules: Vec<Module> = dir_files
            .into_iter()
            .filter(|(_, files)| files.len() >= self.min_files)
            .map(|(path, files)| {
                let name = self.path_to_module_name(&path);
                let module_type = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(ModuleType::from_name)
                    .unwrap_or(ModuleType::Generic);
                let is_package = dir_has_init.get(&path).copied().unwrap_or(false);

                Module {
                    name,
                    path,
                    files,
                    is_package,
                    module_type,
                    outgoing_imports: 0,
                    incoming_imports: 0,
                }
            })
            .collect();

        // Sort by path for consistent ordering
        modules.sort_by(|a, b| a.path.cmp(&b.path));

        modules
    }

    /// Calculate coupling metrics between modules
    pub fn calculate_coupling(&self, modules: &mut [Module], graph: &CodeGraph) {
        // Build a map from file path to module index
        let mut file_to_module: HashMap<PathBuf, usize> = HashMap::new();
        for (idx, module) in modules.iter().enumerate() {
            for &file_id in &module.files {
                if let Some(file_node) = graph.get_file(file_id) {
                    file_to_module.insert(file_node.path.clone(), idx);
                }
            }
        }

        // Count imports between modules
        let mut outgoing: Vec<usize> = vec![0; modules.len()];
        let mut incoming: Vec<usize> = vec![0; modules.len()];

        for (idx, module) in modules.iter().enumerate() {
            let mut imported_modules: HashSet<usize> = HashSet::new();

            for &file_id in &module.files {
                if let Some(file_node) = graph.get_file(file_id) {
                    // Check each import's resolved module
                    for import in &file_node.imports {
                        // Try to find the imported file in our modules
                        let import_path = PathBuf::from(import.module.replace('.', "/"));
                        
                        // Check various possible paths
                        for ext in ["", ".py", "/__init__.py"] {
                            let check_path = if ext.is_empty() {
                                import_path.clone()
                            } else if ext == ".py" {
                                import_path.with_extension("py")
                            } else {
                                import_path.join("__init__.py")
                            };

                            if let Some(&target_module_idx) = file_to_module.get(&check_path) {
                                if target_module_idx != idx {
                                    imported_modules.insert(target_module_idx);
                                }
                            }
                        }
                    }
                }
            }

            outgoing[idx] = imported_modules.len();
            for &target_idx in &imported_modules {
                incoming[target_idx] += 1;
            }
        }

        // Update modules with coupling metrics
        for (idx, module) in modules.iter_mut().enumerate() {
            module.outgoing_imports = outgoing[idx];
            module.incoming_imports = incoming[idx];
        }
    }

    /// Convert a path to a Python module name
    fn path_to_module_name(&self, path: &Path) -> String {
        let components: Vec<&str> = path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        if components.is_empty() {
            "root".to_string()
        } else {
            components.join(".")
        }
    }

    /// Find the root modules (top-level directories with Python files)
    pub fn find_root_modules<'a>(&self, modules: &'a [Module]) -> Vec<&'a Module> {
        modules
            .iter()
            .filter(|m| {
                // Root modules have no dots in their name (single component)
                !m.name.contains('.') && m.name != "root"
            })
            .collect()
    }

    /// Find modules by type
    pub fn find_by_type<'a>(&self, modules: &'a [Module], module_type: ModuleType) -> Vec<&'a Module> {
        modules
            .iter()
            .filter(|m| m.module_type == module_type)
            .collect()
    }
}

impl Default for ModuleDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for detected modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleStats {
    pub total_modules: usize,
    pub total_packages: usize,
    pub by_type: HashMap<String, usize>,
    pub avg_files_per_module: f64,
    pub max_coupling: f64,
}

impl ModuleStats {
    /// Calculate stats from a list of modules
    pub fn from_modules(modules: &[Module]) -> Self {
        let total_modules = modules.len();
        let total_packages = modules.iter().filter(|m| m.is_package).count();
        
        let mut by_type: HashMap<String, usize> = HashMap::new();
        for module in modules {
            *by_type.entry(format!("{:?}", module.module_type)).or_default() += 1;
        }

        let total_files: usize = modules.iter().map(|m| m.files.len()).sum();
        let avg_files_per_module = if total_modules > 0 {
            total_files as f64 / total_modules as f64
        } else {
            0.0
        };

        let max_coupling = modules
            .iter()
            .map(|m| m.coupling_score())
            .fold(0.0, f64::max);

        Self {
            total_modules,
            total_packages,
            by_type,
            avg_files_per_module,
            max_coupling,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ParsedFile;

    fn make_parsed_file(path: &str) -> ParsedFile {
        let path = PathBuf::from(path);
        let module_name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        ParsedFile::new(path, module_name)
    }

    #[test]
    fn test_module_type_detection() {
        assert_eq!(ModuleType::from_name("models"), ModuleType::Models);
        assert_eq!(ModuleType::from_name("Models"), ModuleType::Models);
        assert_eq!(ModuleType::from_name("views"), ModuleType::Views);
        assert_eq!(ModuleType::from_name("services"), ModuleType::Services);
        assert_eq!(ModuleType::from_name("utils"), ModuleType::Utils);
        assert_eq!(ModuleType::from_name("api"), ModuleType::Api);
        assert_eq!(ModuleType::from_name("tests"), ModuleType::Tests);
        assert_eq!(ModuleType::from_name("config"), ModuleType::Config);
        assert_eq!(ModuleType::from_name("core"), ModuleType::Core);
        assert_eq!(ModuleType::from_name("random"), ModuleType::Generic);
    }

    #[test]
    fn test_detect_modules() {
        let mut graph = CodeGraph::new();
        
        // Add files in different directories
        graph.add_file(&make_parsed_file("src/models/user.py"));
        graph.add_file(&make_parsed_file("src/models/post.py"));
        graph.add_file(&make_parsed_file("src/utils/helpers.py"));
        graph.add_file(&make_parsed_file("src/main.py"));
        
        let detector = ModuleDetector::new();
        let modules = detector.detect(&graph, Path::new(""));
        
        assert!(modules.len() >= 2); // At least models and utils
    }

    #[test]
    fn test_find_by_type() {
        let modules = vec![
            Module {
                name: "models".to_string(),
                path: PathBuf::from("models"),
                files: vec![FileId(0)],
                is_package: true,
                module_type: ModuleType::Models,
                outgoing_imports: 0,
                incoming_imports: 0,
            },
            Module {
                name: "utils".to_string(),
                path: PathBuf::from("utils"),
                files: vec![FileId(1)],
                is_package: true,
                module_type: ModuleType::Utils,
                outgoing_imports: 0,
                incoming_imports: 0,
            },
        ];
        
        let detector = ModuleDetector::new();
        let model_modules = detector.find_by_type(&modules, ModuleType::Models);
        
        assert_eq!(model_modules.len(), 1);
        assert_eq!(model_modules[0].name, "models");
    }

    #[test]
    fn test_module_coupling_score() {
        let module = Module {
            name: "test".to_string(),
            path: PathBuf::from("test"),
            files: vec![FileId(0), FileId(1)],
            is_package: false,
            module_type: ModuleType::Generic,
            outgoing_imports: 4,
            incoming_imports: 2,
        };
        
        // (4 + 2) / 2 files = 3.0
        assert_eq!(module.coupling_score(), 3.0);
    }

    #[test]
    fn test_module_stats() {
        let modules = vec![
            Module {
                name: "models".to_string(),
                path: PathBuf::from("models"),
                files: vec![FileId(0), FileId(1)],
                is_package: true,
                module_type: ModuleType::Models,
                outgoing_imports: 2,
                incoming_imports: 4,
            },
            Module {
                name: "utils".to_string(),
                path: PathBuf::from("utils"),
                files: vec![FileId(2)],
                is_package: false,
                module_type: ModuleType::Utils,
                outgoing_imports: 1,
                incoming_imports: 0,
            },
        ];
        
        let stats = ModuleStats::from_modules(&modules);
        
        assert_eq!(stats.total_modules, 2);
        assert_eq!(stats.total_packages, 1);
        assert_eq!(stats.avg_files_per_module, 1.5); // 3 files / 2 modules
    }

    #[test]
    fn test_path_to_module_name() {
        let detector = ModuleDetector::new();
        
        assert_eq!(
            detector.path_to_module_name(Path::new("src/models")),
            "src.models"
        );
        assert_eq!(
            detector.path_to_module_name(Path::new("utils")),
            "utils"
        );
        assert_eq!(
            detector.path_to_module_name(Path::new("")),
            "root"
        );
    }
}
