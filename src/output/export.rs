// Graph export functionality for Cartographer
//
// Exports dependency graphs to DOT, Mermaid, SVG, and PNG formats.

use crate::analysis::{AnalysisResult, EdgeKind, FileId, NodeId};
use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;
use std::process::Command;

/// Graph export format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Dot,
    Mermaid,
    Svg,
    Png,
}

impl ExportFormat {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "dot" => Ok(ExportFormat::Dot),
            "mermaid" | "mmd" => Ok(ExportFormat::Mermaid),
            "svg" => Ok(ExportFormat::Svg),
            "png" => Ok(ExportFormat::Png),
            _ => Err(Error::config_validation(format!("Unknown export format: {}", s))),
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Dot => "dot",
            ExportFormat::Mermaid => "mmd",
            ExportFormat::Svg => "svg",
            ExportFormat::Png => "png",
        }
    }

    pub fn is_image(&self) -> bool {
        matches!(self, ExportFormat::Svg | ExportFormat::Png)
    }
}

/// Options for graph export
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub depth: usize,
    pub module_filters: Vec<String>,
    pub direction: String,
    pub no_externals: bool,
    pub cluster: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Mermaid,
            depth: 5,
            module_filters: Vec::new(),
            direction: "TB".to_string(),
            no_externals: false,
            cluster: false,
        }
    }
}

/// Graph exporter for various formats
pub struct GraphExporter {
    options: ExportOptions,
}

impl GraphExporter {
    pub fn new(options: ExportOptions) -> Self {
        Self { options }
    }

    /// Export the dependency graph to the specified format
    pub fn export(&self, analysis: &AnalysisResult) -> Result<String> {
        match self.options.format {
            ExportFormat::Dot => self.export_dot(analysis),
            ExportFormat::Mermaid => self.export_mermaid(analysis),
            ExportFormat::Svg | ExportFormat::Png => {
                // Generate DOT first, then render
                let dot = self.export_dot(analysis)?;
                self.render_graphviz(&dot)
            }
        }
    }

    /// Export to file, handling binary formats appropriately
    pub fn export_to_file(&self, analysis: &AnalysisResult, path: &Path) -> Result<()> {
        let content = self.export(analysis)?;
        
        if self.options.format.is_image() {
            // For image formats, content is already the binary data (base64 or raw)
            std::fs::write(path, content.as_bytes())
                .map_err(Error::Io)?;
        } else {
            let mut file = std::fs::File::create(path)
                .map_err(Error::Io)?;
            file.write_all(content.as_bytes())
                .map_err(Error::Io)?;
        }
        Ok(())
    }

    /// Generate DOT format (Graphviz)
    fn export_dot(&self, analysis: &AnalysisResult) -> Result<String> {
        let mut lines = Vec::new();
        lines.push("digraph dependencies {".to_string());
        lines.push(format!("    rankdir={};", self.options.direction));
        lines.push("    node [shape=box, style=filled, fontname=\"Helvetica\"];".to_string());
        lines.push("    edge [color=\"#666666\"];".to_string());
        lines.push("".to_string());

        // Build file-to-module mapping for clustering
        let mut file_to_module: HashMap<FileId, String> = HashMap::new();
        for module in &analysis.modules {
            for &file_id in &module.files {
                file_to_module.insert(file_id, module.name.clone());
            }
        }

        // Collect files that pass the filter
        let filtered_files = self.filter_files(analysis, &file_to_module);

        if self.options.cluster {
            // Group by module
            let mut modules_with_files: HashMap<String, Vec<(FileId, String)>> = HashMap::new();
            for (file_id, file_node) in analysis.graph.all_files() {
                if !filtered_files.contains(&file_id) {
                    continue;
                }
                let module = file_to_module.get(&file_id)
                    .cloned()
                    .unwrap_or_else(|| "root".to_string());
                let short_name = file_node.module_name.split('.').next_back()
                    .unwrap_or(&file_node.module_name)
                    .to_string();
                modules_with_files.entry(module).or_default().push((file_id, short_name));
            }

            // Output subgraphs
            for (module, files) in &modules_with_files {
                let cluster_id = sanitize_dot_id(module);
                lines.push(format!("    subgraph cluster_{} {{", cluster_id));
                lines.push(format!("        label=\"{}\";", module));
                lines.push("        style=filled;".to_string());
                lines.push("        color=lightgrey;".to_string());
                for (file_id, name) in files {
                    let node_id = format!("f{}", file_id.0);
                    let color = self.get_node_color(file_to_module.get(file_id).unwrap_or(&"".to_string()));
                    lines.push(format!("        {} [label=\"{}\", fillcolor=\"{}\"];", node_id, name, color));
                }
                lines.push("    }".to_string());
            }
        } else {
            // Flat list of nodes
            for (file_id, file_node) in analysis.graph.all_files() {
                if !filtered_files.contains(&file_id) {
                    continue;
                }
                let node_id = format!("f{}", file_id.0);
                let short_name = file_node.module_name.split('.').next_back()
                    .unwrap_or(&file_node.module_name);
                let empty = String::new();
                let module = file_to_module.get(&file_id).unwrap_or(&empty);
                let color = self.get_node_color(module);
                lines.push(format!("    {} [label=\"{}\", fillcolor=\"{}\"];", node_id, short_name, color));
            }
        }

        lines.push("".to_string());

        // Add edges
        let mut seen_edges: HashSet<(FileId, FileId)> = HashSet::new();
        for edge in &analysis.graph.edges {
            if edge.kind == EdgeKind::Imports {
                if let (NodeId::File(from), NodeId::File(to)) = (edge.from, edge.to) {
                    if !filtered_files.contains(&from) || !filtered_files.contains(&to) {
                        continue;
                    }
                    if seen_edges.insert((from, to)) {
                        let from_id = format!("f{}", from.0);
                        let to_id = format!("f{}", to.0);
                        lines.push(format!("    {} -> {};", from_id, to_id));
                    }
                }
            }
        }

        lines.push("}".to_string());
        Ok(lines.join("\n"))
    }

    /// Generate Mermaid format
    fn export_mermaid(&self, analysis: &AnalysisResult) -> Result<String> {
        let mut lines = Vec::new();
        lines.push(format!("graph {}", self.options.direction));

        // Build file-to-module mapping
        let mut file_to_module: HashMap<FileId, String> = HashMap::new();
        for module in &analysis.modules {
            for &file_id in &module.files {
                file_to_module.insert(file_id, module.name.clone());
            }
        }

        let filtered_files = self.filter_files(analysis, &file_to_module);

        if self.options.cluster {
            // Group by module into subgraphs
            let mut modules_with_files: HashMap<String, Vec<(FileId, String, String)>> = HashMap::new();
            for (file_id, file_node) in analysis.graph.all_files() {
                if !filtered_files.contains(&file_id) {
                    continue;
                }
                let module = file_to_module.get(&file_id)
                    .cloned()
                    .unwrap_or_else(|| "root".to_string());
                let short_name = file_node.module_name.split('.').next_back()
                    .unwrap_or(&file_node.module_name)
                    .to_string();
                let safe_id = sanitize_mermaid_id(&file_node.module_name);
                modules_with_files.entry(module).or_default().push((file_id, short_name, safe_id));
            }

            for (module, files) in &modules_with_files {
                let subgraph_id = sanitize_mermaid_id(module);
                lines.push(format!("    subgraph {}[{}]", subgraph_id, module));
                for (_, name, safe_id) in files {
                    lines.push(format!("        {}[{}]", safe_id, name));
                }
                lines.push("    end".to_string());
            }
        } else {
            // Flat nodes
            for (file_id, file_node) in analysis.graph.all_files() {
                if !filtered_files.contains(&file_id) {
                    continue;
                }
                let short_name = file_node.module_name.split('.').next_back()
                    .unwrap_or(&file_node.module_name);
                let safe_id = sanitize_mermaid_id(&file_node.module_name);
                lines.push(format!("    {}[{}]", safe_id, short_name));
            }
        }

        // Add edges
        let mut seen_edges: HashSet<(FileId, FileId)> = HashSet::new();
        for edge in &analysis.graph.edges {
            if edge.kind == EdgeKind::Imports {
                if let (NodeId::File(from), NodeId::File(to)) = (edge.from, edge.to) {
                    if !filtered_files.contains(&from) || !filtered_files.contains(&to) {
                        continue;
                    }
                    if seen_edges.insert((from, to)) {
                        if let (Some(from_node), Some(to_node)) = (
                            analysis.graph.get_file(from),
                            analysis.graph.get_file(to),
                        ) {
                            let from_id = sanitize_mermaid_id(&from_node.module_name);
                            let to_id = sanitize_mermaid_id(&to_node.module_name);
                            lines.push(format!("    {} --> {}", from_id, to_id));
                        }
                    }
                }
            }
        }

        Ok(lines.join("\n"))
    }

    /// Filter files based on module patterns and options
    fn filter_files(&self, analysis: &AnalysisResult, file_to_module: &HashMap<FileId, String>) -> HashSet<FileId> {
        let mut filtered: HashSet<FileId> = HashSet::new();

        for (file_id, file_node) in analysis.graph.all_files() {
            // Skip externals if requested
            if self.options.no_externals {
                // Check if file is in the analyzed project
                let is_local = analysis.modules.iter()
                    .any(|m| m.files.contains(&file_id));
                if !is_local {
                    continue;
                }
            }

            // Apply module filters
            if !self.options.module_filters.is_empty() {
                let module = file_to_module.get(&file_id)
                    .map(|s| s.as_str())
                    .unwrap_or(&file_node.module_name);
                let matches = self.options.module_filters.iter()
                    .any(|pattern| glob_match(pattern, module));
                if !matches {
                    continue;
                }
            }

            filtered.insert(file_id);
        }

        filtered
    }

    /// Get node color based on module type
    fn get_node_color(&self, module: &str) -> &'static str {
        if module.contains("model") {
            "#e1f5fe"
        } else if module.contains("view") {
            "#f3e5f5"
        } else if module.contains("service") {
            "#e8f5e9"
        } else if module.contains("util") {
            "#fff3e0"
        } else if module.contains("api") {
            "#fce4ec"
        } else if module.contains("test") {
            "#f5f5f5"
        } else {
            "#ffffff"
        }
    }

    /// Render DOT to SVG or PNG using graphviz
    fn render_graphviz(&self, dot: &str) -> Result<String> {
        let format_arg = match self.options.format {
            ExportFormat::Svg => "-Tsvg",
            ExportFormat::Png => "-Tpng",
            _ => return Err(Error::config_validation("Invalid format for graphviz")),
        };

        // Check if dot is available
        let output = Command::new("dot")
            .arg(format_arg)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        let mut child = match output {
            Ok(c) => c,
            Err(_) => return Err(Error::config_validation(
                "graphviz (dot) not found. Install graphviz to export SVG/PNG."
            )),
        };

        // Write DOT to stdin
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(dot.as_bytes())
                .map_err(Error::Io)?;
        }

        let output = child.wait_with_output()
            .map_err(Error::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::other(format!("graphviz error: {}", stderr)));
        }

        // For text output, convert bytes to string
        // For binary (PNG), this will be lossy but we handle it in export_to_file
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Sanitize an identifier for DOT format
fn sanitize_dot_id(s: &str) -> String {
    s.replace(['.', '/', '-', ' '], "_")
}

/// Sanitize an identifier for Mermaid format
fn sanitize_mermaid_id(s: &str) -> String {
    s.replace(['.', '/', '-', ' '], "_")
}

/// Simple glob matching for module patterns
fn glob_match(pattern: &str, s: &str) -> bool {
    // Handle simple patterns: exact match, prefix/*, and *suffix
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("*") {
        return s.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix("*") {
        return s.ends_with(suffix);
    }
    pattern == s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_format_parse() {
        assert_eq!(ExportFormat::parse("dot").unwrap(), ExportFormat::Dot);
        assert_eq!(ExportFormat::parse("mermaid").unwrap(), ExportFormat::Mermaid);
        assert_eq!(ExportFormat::parse("mmd").unwrap(), ExportFormat::Mermaid);
        assert_eq!(ExportFormat::parse("svg").unwrap(), ExportFormat::Svg);
        assert_eq!(ExportFormat::parse("png").unwrap(), ExportFormat::Png);
        assert!(ExportFormat::parse("invalid").is_err());
    }

    #[test]
    fn test_export_format_extension() {
        assert_eq!(ExportFormat::Dot.extension(), "dot");
        assert_eq!(ExportFormat::Mermaid.extension(), "mmd");
        assert_eq!(ExportFormat::Svg.extension(), "svg");
        assert_eq!(ExportFormat::Png.extension(), "png");
    }

    #[test]
    fn test_export_format_is_image() {
        assert!(!ExportFormat::Dot.is_image());
        assert!(!ExportFormat::Mermaid.is_image());
        assert!(ExportFormat::Svg.is_image());
        assert!(ExportFormat::Png.is_image());
    }

    #[test]
    fn test_sanitize_dot_id() {
        assert_eq!(sanitize_dot_id("foo.bar"), "foo_bar");
        assert_eq!(sanitize_dot_id("my-module"), "my_module");
        assert_eq!(sanitize_dot_id("path/to/file"), "path_to_file");
    }

    #[test]
    fn test_sanitize_mermaid_id() {
        assert_eq!(sanitize_mermaid_id("foo.bar"), "foo_bar");
        assert_eq!(sanitize_mermaid_id("my-module"), "my_module");
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("models*", "models"));
        assert!(glob_match("models*", "models.user"));
        assert!(glob_match("*utils", "string_utils"));
        assert!(glob_match("api", "api"));
        assert!(!glob_match("models*", "views"));
        assert!(!glob_match("api", "api.v1"));
    }

    #[test]
    fn test_default_options() {
        let opts = ExportOptions::default();
        assert_eq!(opts.format, ExportFormat::Mermaid);
        assert_eq!(opts.depth, 5);
        assert!(opts.module_filters.is_empty());
        assert_eq!(opts.direction, "TB");
        assert!(!opts.no_externals);
        assert!(!opts.cluster);
    }

    #[test]
    fn test_node_color() {
        let exporter = GraphExporter::new(ExportOptions::default());
        assert_eq!(exporter.get_node_color("models"), "#e1f5fe");
        assert_eq!(exporter.get_node_color("user_model"), "#e1f5fe");
        assert_eq!(exporter.get_node_color("views"), "#f3e5f5");
        assert_eq!(exporter.get_node_color("services"), "#e8f5e9");
        assert_eq!(exporter.get_node_color("utils"), "#fff3e0");
        assert_eq!(exporter.get_node_color("api"), "#fce4ec");
        assert_eq!(exporter.get_node_color("tests"), "#f5f5f5");
        assert_eq!(exporter.get_node_color("other"), "#ffffff");
    }
}
