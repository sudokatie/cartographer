// Template engine for generating HTML output

use crate::analysis::{AnalysisResult, ClassNode, FileNode, FunctionNode, Module};
use crate::error::Result;
use serde::Serialize;
use std::collections::HashMap;
use tera::{Context, Tera, Value};

/// Template engine wrapping Tera with custom filters and templates
pub struct TemplateEngine {
    tera: Tera,
}

impl TemplateEngine {
    /// Create a new template engine with embedded templates
    pub fn new() -> Result<Self> {
        let mut tera = Tera::default();
        
        // Add embedded templates
        tera.add_raw_templates(vec![
            ("base.html", include_str!("../../templates/base.html.tera")),
            ("index.html", include_str!("../../templates/index.html.tera")),
            ("module.html", include_str!("../../templates/module.html.tera")),
            ("class.html", include_str!("../../templates/class.html.tera")),
            ("function.html", include_str!("../../templates/function.html.tera")),
        ])?;
        
        // Register custom filters
        tera.register_filter("truncate_words", truncate_words);
        tera.register_filter("pluralize", pluralize);
        tera.register_filter("code_highlight", code_highlight);
        tera.register_filter("slugify", slugify_filter);
        
        Ok(Self { tera })
    }
    
    /// Create a template engine from a custom directory
    pub fn from_dir(template_dir: &str) -> Result<Self> {
        let pattern = format!("{}/**/*.tera", template_dir);
        let mut tera = Tera::new(&pattern)?;
        
        // Register custom filters
        tera.register_filter("truncate_words", truncate_words);
        tera.register_filter("pluralize", pluralize);
        tera.register_filter("code_highlight", code_highlight);
        tera.register_filter("slugify", slugify_filter);
        
        Ok(Self { tera })
    }
    
    /// Render the index page
    pub fn render_index(&self, analysis: &AnalysisResult, project_name: &str) -> Result<String> {
        let mut context = Context::new();
        context.insert("project_name", project_name);
        context.insert("stats", &analysis.graph.stats());
        context.insert("modules", &analysis.modules);
        context.insert("metrics", &analysis.metrics);
        
        Ok(self.tera.render("index.html", &context)?)
    }
    
    /// Render a module page
    pub fn render_module(&self, module: &Module, analysis: &AnalysisResult) -> Result<String> {
        let mut context = Context::new();
        context.insert("module", module);
        
        // Collect files in this module
        let files: Vec<&FileNode> = module
            .files
            .iter()
            .filter_map(|id| analysis.graph.get_file(*id))
            .collect();
        context.insert("files", &files);
        
        // Collect classes and functions
        let mut classes: Vec<&ClassNode> = Vec::new();
        let mut functions: Vec<&FunctionNode> = Vec::new();
        
        for file in &files {
            for class_id in &file.classes {
                if let Some(class) = analysis.graph.get_class(*class_id) {
                    classes.push(class);
                }
            }
            for func_id in &file.functions {
                if let Some(func) = analysis.graph.get_function(*func_id) {
                    functions.push(func);
                }
            }
        }
        
        context.insert("classes", &classes);
        context.insert("functions", &functions);
        
        Ok(self.tera.render("module.html", &context)?)
    }
    
    /// Render a class page
    pub fn render_class(&self, class: &ClassNode, analysis: &AnalysisResult) -> Result<String> {
        let mut context = Context::new();
        context.insert("class", class);
        
        // Get methods
        let methods: Vec<&FunctionNode> = class
            .methods
            .iter()
            .filter_map(|id| analysis.graph.get_function(*id))
            .collect();
        context.insert("methods", &methods);
        
        // Get file info
        if let Some(file) = analysis.graph.get_file(class.file) {
            context.insert("file", file);
        }
        
        Ok(self.tera.render("class.html", &context)?)
    }
    
    /// Render a function page
    pub fn render_function(&self, func: &FunctionNode, analysis: &AnalysisResult) -> Result<String> {
        let mut context = Context::new();
        context.insert("function", func);
        
        // Get file info
        if let Some(file) = analysis.graph.get_file(func.file) {
            context.insert("file", file);
        }
        
        // Get class info if it's a method
        if let Some(class_id) = func.class {
            if let Some(class) = analysis.graph.get_class(class_id) {
                context.insert("class", class);
            }
        }
        
        Ok(self.tera.render("function.html", &context)?)
    }
    
    /// Render a custom template with context
    pub fn render(&self, template_name: &str, context: &Context) -> Result<String> {
        Ok(self.tera.render(template_name, context)?)
    }
    
    /// Get the underlying Tera instance for advanced usage
    pub fn tera(&self) -> &Tera {
        &self.tera
    }
}

/// Truncate text to a number of words
fn truncate_words(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let s = value.as_str().unwrap_or("");
    let max_words = args
        .get("count")
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as usize;
    
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.len() <= max_words {
        Ok(Value::String(s.to_string()))
    } else {
        let truncated: String = words[..max_words].join(" ");
        Ok(Value::String(format!("{}...", truncated)))
    }
}

/// Pluralize a word based on count
fn pluralize(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
    let count = value.as_u64().unwrap_or(0);
    let singular = args
        .get("singular")
        .and_then(|v| v.as_str())
        .unwrap_or("item");
    let default_plural = format!("{}s", singular);
    let plural = args
        .get("plural")
        .and_then(|v| v.as_str())
        .unwrap_or(&default_plural);
    
    if count == 1 {
        Ok(Value::String(format!("{} {}", count, singular)))
    } else {
        Ok(Value::String(format!("{} {}", count, plural)))
    }
}

/// Simple code highlighting (placeholder - could use syntect)
fn code_highlight(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    let code = value.as_str().unwrap_or("");
    // For now, just wrap in pre/code tags
    Ok(Value::String(format!(
        "<pre><code class=\"language-python\">{}</code></pre>",
        html_escape(code)
    )))
}

/// Convert text to URL-friendly slug
fn slugify_filter(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    let s = value.as_str().unwrap_or("");
    Ok(Value::String(slugify(s)))
}

/// Convert text to URL-friendly slug
pub fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Escape HTML special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Context for rendering the search index
#[derive(Debug, Serialize)]
pub struct SearchEntry {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub description: Option<String>,
    pub module: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("MyClass.my_method"), "myclass-my-method");
        assert_eq!(slugify("  spaced  out  "), "spaced-out");
        assert_eq!(slugify("CamelCase"), "camelcase");
    }
    
    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<div>"), "&lt;div&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }
    
    #[test]
    fn test_truncate_words() {
        let value = Value::String("one two three four five".to_string());
        let mut args = HashMap::new();
        args.insert("count".to_string(), Value::Number(3.into()));
        
        let result = truncate_words(&value, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "one two three...");
    }
    
    #[test]
    fn test_truncate_words_no_truncation() {
        let value = Value::String("one two".to_string());
        let mut args = HashMap::new();
        args.insert("count".to_string(), Value::Number(5.into()));
        
        let result = truncate_words(&value, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "one two");
    }
    
    #[test]
    fn test_pluralize_singular() {
        let value = Value::Number(1.into());
        let mut args = HashMap::new();
        args.insert("singular".to_string(), Value::String("file".to_string()));
        
        let result = pluralize(&value, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "1 file");
    }
    
    #[test]
    fn test_pluralize_plural() {
        let value = Value::Number(5.into());
        let mut args = HashMap::new();
        args.insert("singular".to_string(), Value::String("file".to_string()));
        
        let result = pluralize(&value, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "5 files");
    }
    
    #[test]
    fn test_pluralize_zero() {
        let value = Value::Number(0.into());
        let mut args = HashMap::new();
        args.insert("singular".to_string(), Value::String("item".to_string()));
        
        let result = pluralize(&value, &args).unwrap();
        assert_eq!(result.as_str().unwrap(), "0 items");
    }
    
    #[test]
    fn test_code_highlight() {
        let value = Value::String("def foo(): pass".to_string());
        let args = HashMap::new();
        
        let result = code_highlight(&value, &args).unwrap();
        let html = result.as_str().unwrap();
        assert!(html.contains("<pre><code"));
        assert!(html.contains("def foo(): pass"));
    }
}
