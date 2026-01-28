// Diagram generation for Cartographer
//
// Generates Mermaid diagrams for dependency visualization.

use crate::analysis::{AnalysisResult, EdgeKind, FileId, Module, NodeId};
use std::collections::{HashMap, HashSet};

/// Diagram generator for creating Mermaid diagrams
pub struct DiagramGenerator {
    /// Maximum nodes to display before aggregating
    max_nodes: usize,
    /// Layout direction (TB, LR, BT, RL)
    direction: String,
}

impl DiagramGenerator {
    /// Create a new diagram generator
    pub fn new() -> Self {
        Self {
            max_nodes: 100,
            direction: "TB".to_string(),
        }
    }

    /// Set maximum nodes before aggregation
    pub fn with_max_nodes(mut self, max: usize) -> Self {
        self.max_nodes = max;
        self
    }

    /// Set layout direction
    pub fn with_direction(mut self, dir: &str) -> Self {
        self.direction = dir.to_string();
        self
    }

    /// Generate a full dependency graph showing all file imports
    pub fn generate_dependency_graph(&self, analysis: &AnalysisResult) -> String {
        let mut lines = Vec::new();
        lines.push(format!("graph {}", self.direction));

        // Collect all files and their short names
        let mut file_names: HashMap<FileId, String> = HashMap::new();
        for (file_id, file_node) in analysis.graph.all_files() {
            let short_name = file_node
                .module_name
                .split('.')
                .next_back()
                .unwrap_or(&file_node.module_name);
            file_names.insert(file_id, short_name.to_string());
        }

        // If too many nodes, aggregate by module
        if file_names.len() > self.max_nodes {
            return self.generate_module_level_graph(analysis);
        }

        // Add nodes with styling based on module type
        for (file_id, file_node) in analysis.graph.all_files() {
            let name = file_names.get(&file_id).unwrap();
            let safe_id = sanitize_id(&file_node.module_name);
            let style = self.get_node_style(&file_node.module_name);
            lines.push(format!("    {}[{}]{}", safe_id, name, style));
        }

        // Add edges
        let mut seen_edges: HashSet<(FileId, FileId)> = HashSet::new();
        for edge in &analysis.graph.edges {
            if edge.kind == EdgeKind::Imports {
                if let (NodeId::File(from), NodeId::File(to)) = (edge.from, edge.to) {
                    if seen_edges.insert((from, to)) {
                        if let (Some(from_node), Some(to_node)) = (
                            analysis.graph.get_file(from),
                            analysis.graph.get_file(to),
                        ) {
                            let from_id = sanitize_id(&from_node.module_name);
                            let to_id = sanitize_id(&to_node.module_name);
                            lines.push(format!("    {} --> {}", from_id, to_id));
                        }
                    }
                }
            }
        }

        lines.join("\n")
    }

    /// Generate a module-level dependency graph (aggregated view)
    pub fn generate_module_level_graph(&self, analysis: &AnalysisResult) -> String {
        let mut lines = Vec::new();
        lines.push(format!("graph {}", self.direction));

        // Create subgraphs for each module
        let mut module_edges: HashSet<(String, String)> = HashSet::new();

        // Build file-to-module mapping
        let mut file_to_module: HashMap<FileId, String> = HashMap::new();
        for module in &analysis.modules {
            for &file_id in &module.files {
                file_to_module.insert(file_id, module.name.clone());
            }
        }

        // Add module nodes
        for module in &analysis.modules {
            let safe_id = sanitize_id(&module.name);
            let file_count = module.files.len();
            lines.push(format!(
                "    {}[\"{}\\n({} files)\"]",
                safe_id, module.name, file_count
            ));
        }

        // Add inter-module edges
        for edge in &analysis.graph.edges {
            if edge.kind == EdgeKind::Imports {
                if let (NodeId::File(from), NodeId::File(to)) = (edge.from, edge.to) {
                    if let (Some(from_mod), Some(to_mod)) =
                        (file_to_module.get(&from), file_to_module.get(&to))
                    {
                        if from_mod != to_mod {
                            module_edges.insert((from_mod.clone(), to_mod.clone()));
                        }
                    }
                }
            }
        }

        for (from_mod, to_mod) in module_edges {
            let from_id = sanitize_id(&from_mod);
            let to_id = sanitize_id(&to_mod);
            lines.push(format!("    {} --> {}", from_id, to_id));
        }

        lines.join("\n")
    }

    /// Generate a diagram for a single module showing internal structure
    pub fn generate_module_graph(&self, module: &Module, analysis: &AnalysisResult) -> String {
        let mut lines = Vec::new();
        lines.push(format!("graph {}", self.direction));
        lines.push(format!("    subgraph {} [{}]", sanitize_id(&module.name), module.name));

        // Add files in this module
        for &file_id in &module.files {
            if let Some(file_node) = analysis.graph.get_file(file_id) {
                let short_name = file_node
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let safe_id = sanitize_id(&file_node.module_name);
                lines.push(format!("        {}[{}]", safe_id, short_name));
            }
        }

        lines.push("    end".to_string());

        // Add internal imports within the module
        let module_files: HashSet<FileId> = module.files.iter().copied().collect();
        for &file_id in &module.files {
            let imports = analysis.graph.imports_of(file_id);
            for imported in imports {
                if module_files.contains(&imported) {
                    if let (Some(from_node), Some(to_node)) = (
                        analysis.graph.get_file(file_id),
                        analysis.graph.get_file(imported),
                    ) {
                        let from_id = sanitize_id(&from_node.module_name);
                        let to_id = sanitize_id(&to_node.module_name);
                        lines.push(format!("    {} --> {}", from_id, to_id));
                    }
                }
            }
        }

        lines.join("\n")
    }

    /// Generate a class hierarchy diagram
    pub fn generate_class_hierarchy(&self, analysis: &AnalysisResult) -> String {
        let mut lines = Vec::new();
        lines.push("classDiagram".to_string());
        lines.push(format!("    direction {}", self.direction));

        // Add all classes
        for (_, class_node) in analysis.graph.all_classes() {
            let safe_name = sanitize_class_name(&class_node.name);
            lines.push(format!("    class {} {{", safe_name));

            // Add methods (limited to avoid huge diagrams)
            for func_id in class_node.methods.iter().take(5) {
                if let Some(func_node) = analysis.graph.get_function(*func_id) {
                    let visibility = if func_node.name.starts_with('_') {
                        "-"
                    } else {
                        "+"
                    };
                    lines.push(format!("        {}{}()", visibility, func_node.name));
                }
            }
            if class_node.methods.len() > 5 {
                lines.push(format!("        +... {} more", class_node.methods.len() - 5));
            }

            lines.push("    }".to_string());

            // Add inheritance relationships
            for base in &class_node.bases {
                let safe_base = sanitize_class_name(base);
                lines.push(format!("    {} <|-- {}", safe_base, safe_name));
            }
        }

        lines.join("\n")
    }

    /// Get node style based on module path patterns
    fn get_node_style(&self, module_name: &str) -> &'static str {
        let lower = module_name.to_lowercase();
        if lower.contains("test") {
            ":::test"
        } else if lower.contains("model") {
            ":::model"
        } else if lower.contains("util") || lower.contains("helper") {
            ":::util"
        } else if lower.contains("api") || lower.contains("route") {
            ":::api"
        } else if lower.contains("config") || lower.contains("setting") {
            ":::config"
        } else {
            ""
        }
    }
}

impl Default for DiagramGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Sanitize a string for use as a Mermaid node ID
fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Sanitize a class name for Mermaid class diagrams
fn sanitize_class_name(s: &str) -> String {
    // Extract just the class name if it's a dotted path
    let name = s.split('.').next_back().unwrap_or(s);
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("my.module"), "my_module");
        assert_eq!(sanitize_id("MyClass"), "MyClass");
        assert_eq!(sanitize_id("foo-bar"), "foo_bar");
    }

    #[test]
    fn test_sanitize_class_name() {
        assert_eq!(sanitize_class_name("MyClass"), "MyClass");
        assert_eq!(sanitize_class_name("module.MyClass"), "MyClass");
        assert_eq!(sanitize_class_name("foo.bar.Baz"), "Baz");
    }

    #[test]
    fn test_diagram_generator_new() {
        let gen = DiagramGenerator::new();
        assert_eq!(gen.max_nodes, 100);
        assert_eq!(gen.direction, "TB");
    }

    #[test]
    fn test_with_max_nodes() {
        let gen = DiagramGenerator::new().with_max_nodes(50);
        assert_eq!(gen.max_nodes, 50);
    }

    #[test]
    fn test_with_direction() {
        let gen = DiagramGenerator::new().with_direction("LR");
        assert_eq!(gen.direction, "LR");
    }

    #[test]
    fn test_get_node_style() {
        let gen = DiagramGenerator::new();
        assert_eq!(gen.get_node_style("tests.test_main"), ":::test");
        assert_eq!(gen.get_node_style("models.user"), ":::model");
        assert_eq!(gen.get_node_style("utils.helpers"), ":::util");
        assert_eq!(gen.get_node_style("api.routes"), ":::api");
        assert_eq!(gen.get_node_style("core.main"), "");
    }
}
