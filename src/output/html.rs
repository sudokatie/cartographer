// HTML site generator
//
// Writes the static site files to disk: index.html, module pages,
// class pages, and search.json.

use crate::analysis::{AnalysisResult, ClassId, Module};
use crate::error::Result;
use crate::output::templates::{SearchEntry, TemplateEngine};
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration for HTML generation
#[derive(Debug, Clone)]
pub struct HtmlConfig {
    /// Output directory
    pub output_dir: PathBuf,
    /// Project name for titles
    pub project_name: String,
    /// Whether to generate diagrams
    pub generate_diagrams: bool,
    /// Whether to copy assets
    pub copy_assets: bool,
}

impl Default for HtmlConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("cartographer-docs"),
            project_name: "Project".to_string(),
            generate_diagrams: true,
            copy_assets: true,
        }
    }
}

/// HTML site generator
pub struct HtmlGenerator {
    config: HtmlConfig,
    template_engine: TemplateEngine,
}

impl HtmlGenerator {
    /// Create a new HTML generator
    pub fn new(config: HtmlConfig) -> Result<Self> {
        let template_engine = TemplateEngine::new()?;
        Ok(Self {
            config,
            template_engine,
        })
    }

    /// Generate the complete static site
    pub fn generate(&self, analysis: &AnalysisResult) -> Result<GenerationReport> {
        let mut report = GenerationReport::default();

        // Create output directory structure
        self.create_directories()?;

        // Copy assets (CSS, JS)
        if self.config.copy_assets {
            self.copy_assets()?;
            report.assets_copied = true;
        }

        // Generate index page
        self.generate_index(analysis)?;
        report.pages_generated += 1;

        // Generate module pages
        for module in &analysis.modules {
            self.generate_module_page(module, analysis)?;
            report.pages_generated += 1;

            // Generate class pages within module
            for &file_id in &module.files {
                if let Some(file_node) = analysis.graph.get_file(file_id) {
                    for &class_id in &file_node.classes {
                        self.generate_class_page(class_id, module, analysis)?;
                        report.pages_generated += 1;
                    }
                }
            }
        }

        // Generate search index
        self.generate_search_index(analysis)?;
        report.search_index_generated = true;

        // Generate diagrams if enabled
        if self.config.generate_diagrams {
            self.generate_diagrams(analysis)?;
            report.diagrams_generated = true;
        }

        Ok(report)
    }

    /// Create the output directory structure
    fn create_directories(&self) -> Result<()> {
        let dirs = [
            self.config.output_dir.clone(),
            self.config.output_dir.join("modules"),
            self.config.output_dir.join("assets"),
            self.config.output_dir.join("assets/diagrams"),
        ];

        for dir in &dirs {
            fs::create_dir_all(dir)?;
        }

        Ok(())
    }

    /// Copy static assets (CSS, JS)
    fn copy_assets(&self) -> Result<()> {
        let assets_dir = self.config.output_dir.join("assets");

        // Write embedded CSS
        let css_content = include_str!("../../assets/style.css");
        fs::write(assets_dir.join("style.css"), css_content)?;

        // Write embedded JS
        let js_content = include_str!("../../assets/script.js");
        fs::write(assets_dir.join("script.js"), js_content)?;

        Ok(())
    }

    /// Generate the main index page
    fn generate_index(&self, analysis: &AnalysisResult) -> Result<()> {
        let html = self
            .template_engine
            .render_index(analysis, &self.config.project_name)?;

        let path = self.config.output_dir.join("index.html");
        fs::write(&path, html)?;

        Ok(())
    }

    /// Generate a module page
    fn generate_module_page(&self, module: &Module, analysis: &AnalysisResult) -> Result<()> {
        let html = self.template_engine.render_module(module, analysis)?;

        let module_dir = self
            .config
            .output_dir
            .join("modules")
            .join(slugify(&module.name));
        fs::create_dir_all(&module_dir)?;

        let path = module_dir.join("index.html");
        fs::write(&path, html)?;

        Ok(())
    }

    /// Generate a class page
    fn generate_class_page(
        &self,
        class_id: ClassId,
        module: &Module,
        analysis: &AnalysisResult,
    ) -> Result<()> {
        if let Some(class_node) = analysis.graph.get_class(class_id) {
            let html = self.template_engine.render_class(class_node, analysis)?;

            let module_dir = self
                .config
                .output_dir
                .join("modules")
                .join(slugify(&module.name));

            let path = module_dir.join(format!("{}.html", slugify(&class_node.name)));
            fs::write(&path, html)?;
        }

        Ok(())
    }

    /// Generate the search index (search.json)
    fn generate_search_index(&self, analysis: &AnalysisResult) -> Result<()> {
        let mut entries: Vec<SearchEntry> = Vec::new();

        // Add modules
        for module in &analysis.modules {
            entries.push(SearchEntry {
                name: module.name.clone(),
                kind: "module".to_string(),
                path: format!("modules/{}/index.html", slugify(&module.name)),
                description: None,
                module: module.name.clone(),
            });
        }

        // Add files
        for (_, file_node) in analysis.graph.all_files() {
            entries.push(SearchEntry {
                name: file_node.module_name.clone(),
                kind: "file".to_string(),
                path: format!(
                    "modules/{}/index.html",
                    slugify(file_node.module_name.split('.').next().unwrap_or("root"))
                ),
                description: file_node.docstring.clone(),
                module: file_node.module_name.clone(),
            });
        }

        // Add classes
        for (_, class_node) in analysis.graph.all_classes() {
            if let Some(file_node) = analysis.graph.get_file(class_node.file) {
                let module_name = file_node
                    .module_name
                    .split('.')
                    .next()
                    .unwrap_or("root")
                    .to_string();
                entries.push(SearchEntry {
                    name: class_node.name.clone(),
                    kind: "class".to_string(),
                    path: format!(
                        "modules/{}/{}.html",
                        slugify(&module_name),
                        slugify(&class_node.name)
                    ),
                    description: class_node.docstring.clone(),
                    module: module_name,
                });
            }
        }

        // Add functions
        for (_, func_node) in analysis.graph.all_functions() {
            if func_node.class.is_none() {
                // Only top-level functions, not methods
                if let Some(file_node) = analysis.graph.get_file(func_node.file) {
                    let module_name = file_node
                        .module_name
                        .split('.')
                        .next()
                        .unwrap_or("root")
                        .to_string();
                    entries.push(SearchEntry {
                        name: func_node.name.clone(),
                        kind: "function".to_string(),
                        path: format!("modules/{}/index.html", slugify(&module_name)),
                        description: func_node.docstring.clone(),
                        module: module_name,
                    });
                }
            }
        }

        let json = serde_json::to_string_pretty(&entries)?;
        let path = self.config.output_dir.join("search.json");
        fs::write(&path, json)?;

        Ok(())
    }

    /// Generate diagrams
    fn generate_diagrams(&self, analysis: &AnalysisResult) -> Result<()> {
        use crate::output::diagrams::DiagramGenerator;

        let diagram_gen = DiagramGenerator::new();
        let diagrams_dir = self.config.output_dir.join("assets/diagrams");

        // Generate dependency graph
        let dep_mermaid = diagram_gen.generate_dependency_graph(analysis);
        fs::write(diagrams_dir.join("dependency.mmd"), &dep_mermaid)?;

        // Generate module-level graphs
        for module in &analysis.modules {
            let module_mermaid = diagram_gen.generate_module_graph(module, analysis);
            let filename = format!("{}.mmd", slugify(&module.name));
            fs::write(diagrams_dir.join(filename), &module_mermaid)?;
        }

        Ok(())
    }

    /// Get the output directory
    pub fn output_dir(&self) -> &Path {
        &self.config.output_dir
    }
}

/// Report of what was generated
#[derive(Debug, Default)]
pub struct GenerationReport {
    pub pages_generated: usize,
    pub assets_copied: bool,
    pub search_index_generated: bool,
    pub diagrams_generated: bool,
}

impl GenerationReport {
    pub fn summary(&self) -> String {
        format!(
            "Generated {} pages, assets: {}, search: {}, diagrams: {}",
            self.pages_generated,
            if self.assets_copied { "yes" } else { "no" },
            if self.search_index_generated {
                "yes"
            } else {
                "no"
            },
            if self.diagrams_generated { "yes" } else { "no" }
        )
    }
}

/// Convert text to URL-friendly slug
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("my.module"), "my-module");
        assert_eq!(slugify("MyClass"), "myclass");
    }

    #[test]
    fn test_html_config_default() {
        let config = HtmlConfig::default();
        assert_eq!(config.output_dir, PathBuf::from("cartographer-docs"));
        assert!(config.generate_diagrams);
        assert!(config.copy_assets);
    }

    #[test]
    fn test_create_directories() {
        let dir = TempDir::new().unwrap();
        let config = HtmlConfig {
            output_dir: dir.path().join("docs"),
            ..Default::default()
        };

        let generator = HtmlGenerator::new(config).unwrap();
        generator.create_directories().unwrap();

        assert!(dir.path().join("docs").exists());
        assert!(dir.path().join("docs/modules").exists());
        assert!(dir.path().join("docs/assets").exists());
    }

    #[test]
    fn test_generation_report_summary() {
        let report = GenerationReport {
            pages_generated: 5,
            assets_copied: true,
            search_index_generated: true,
            diagrams_generated: true,
        };

        let summary = report.summary();
        assert!(summary.contains("5 pages"));
        assert!(summary.contains("assets: yes"));
    }
}
