// Rust parser using tree-sitter

use crate::error::{Error, Result};
use crate::parser::ast::{
    Class, Constant, Function, Import, ImportKind, Parameter, ParameterKind, ParsedFile,
};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Parser for Rust source files
pub struct RustParser {
    parser: Parser,
}

impl RustParser {
    /// Create a new Rust parser
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::language();
        parser.set_language(&language).map_err(|e| {
            Error::Parser(format!("Failed to set Rust language: {}", e))
        })?;

        Ok(Self { parser })
    }

    /// Parse a Rust file
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedFile> {
        let source = std::fs::read_to_string(path).map_err(|e| {
            Error::Io(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        let module_name = path_to_module_name(path);
        self.parse_source(&source, path.to_path_buf(), module_name)
    }

    /// Parse Rust source code
    pub fn parse_source(
        &mut self,
        source: &str,
        path: std::path::PathBuf,
        module_name: String,
    ) -> Result<ParsedFile> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            Error::parser("Failed to parse Rust source")
        })?;

        let root = tree.root_node();
        let mut file = ParsedFile::new(path, module_name);

        // Extract module docstring (first inner doc comment)
        file.docstring = extract_module_docstring(&root, source);

        // Walk the tree and extract constructs
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "use_declaration" => {
                    if let Some(import) = parse_use(&child, source) {
                        file.imports.push(import);
                    }
                }
                "struct_item" => {
                    if let Some(class) = parse_struct(&child, source) {
                        file.classes.push(class);
                    }
                }
                "enum_item" => {
                    if let Some(class) = parse_enum(&child, source) {
                        file.classes.push(class);
                    }
                }
                "trait_item" => {
                    if let Some(class) = parse_trait(&child, source) {
                        file.classes.push(class);
                    }
                }
                "impl_item" => {
                    // impl blocks add methods to existing types
                    // We'll handle these by extracting methods
                    if let Some(methods) = parse_impl(&child, source) {
                        for method in methods {
                            file.functions.push(method);
                        }
                    }
                }
                "function_item" => {
                    if let Some(func) = parse_function(&child, source) {
                        file.functions.push(func);
                    }
                }
                "const_item" | "static_item" => {
                    if let Some(constant) = parse_const(&child, source) {
                        file.constants.push(constant);
                    }
                }
                "mod_item" => {
                    // Inline module declaration - extract name for import
                    if let Some(import) = parse_mod(&child, source) {
                        file.imports.push(import);
                    }
                }
                _ => {}
            }
        }

        // Count lines
        let (total, code, comment, _blank) = count_lines(source);
        file.total_lines = total;
        file.code_lines = code;
        file.comment_lines = comment;

        Ok(file)
    }
}

impl Default for RustParser {
    fn default() -> Self {
        Self::new().expect("Failed to create RustParser")
    }
}

/// Convert file path to Rust module name
fn path_to_module_name(path: &Path) -> String {
    let stem = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // mod.rs and lib.rs use parent directory name
    if stem == "mod" || stem == "lib" {
        if let Some(parent) = path.parent() {
            if let Some(name) = parent.file_name().and_then(|s| s.to_str()) {
                return name.to_string();
            }
        }
    }

    stem.to_string()
}

/// Extract module-level docstring from inner doc comments
fn extract_module_docstring(root: &Node, source: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "inner_line_doc" || child.kind() == "inner_block_doc" {
            let text = child.utf8_text(source.as_bytes()).ok()?;
            // Strip the //! or /*! prefix
            let doc = text.trim_start_matches("//!")
                .trim_start_matches("/*!")
                .trim_end_matches("*/")
                .trim();
            if !doc.is_empty() {
                return Some(doc.to_string());
            }
        }
    }
    None
}

/// Parse use declaration
fn parse_use(node: &Node, source: &str) -> Option<Import> {
    // Get the full use statement text and extract the path
    let full_text = node.utf8_text(source.as_bytes()).ok()?;
    
    // Remove "use " prefix and ";" suffix
    let path = full_text.trim()
        .trim_start_matches("pub ")
        .trim_start_matches("use ")
        .trim_end_matches(';')
        .trim();
    
    let module = extract_module_from_use(path);
    
    Some(Import {
        module,
        names: vec![],
        kind: ImportKind::Direct,
        line: node.start_position().row + 1,
    })
}

/// Extract the module path from a use statement
fn extract_module_from_use(use_tree: &str) -> String {
    // Handle patterns like:
    // std::collections::HashMap -> std::collections
    // crate::parser::ast -> crate::parser
    // super::foo -> super
    
    let tree = use_tree.trim();
    
    // Remove braces and get the base path
    let base = if let Some(brace_pos) = tree.find('{') {
        tree[..brace_pos].trim_end_matches("::")
    } else if let Some(as_pos) = tree.find(" as ") {
        tree[..as_pos].rsplit_once("::").map(|(base, _)| base).unwrap_or(tree)
    } else {
        // Get all but last component
        tree.rsplit_once("::").map(|(base, _)| base).unwrap_or(tree)
    };
    
    base.to_string()
}

/// Parse struct definition
fn parse_struct(node: &Node, source: &str) -> Option<Class> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let docstring = extract_outer_doc(node, source);
    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    Some(Class {
        name,
        bases: vec![],
        docstring,
        methods: vec![],
        decorators: extract_attributes(node, source),
        attributes: vec![],
        line_start,
        line_end,
    })
}

/// Parse enum definition
fn parse_enum(node: &Node, source: &str) -> Option<Class> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let docstring = extract_outer_doc(node, source);
    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    Some(Class {
        name,
        bases: vec!["Enum".to_string()], // Mark as enum
        docstring,
        methods: vec![],
        decorators: extract_attributes(node, source),
        attributes: vec![],
        line_start,
        line_end,
    })
}

/// Parse trait definition
fn parse_trait(node: &Node, source: &str) -> Option<Class> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let docstring = extract_outer_doc(node, source);
    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract trait methods
    let mut methods = vec![];
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "function_signature_item" || child.kind() == "function_item" {
                if let Some(method) = parse_function(&child, source) {
                    methods.push(method);
                }
            }
        }
    }

    Some(Class {
        name,
        bases: vec!["Trait".to_string()], // Mark as trait
        docstring,
        methods,
        decorators: extract_attributes(node, source),
        attributes: vec![],
        line_start,
        line_end,
    })
}

/// Parse impl block and extract methods
fn parse_impl(node: &Node, source: &str) -> Option<Vec<Function>> {
    let mut methods = vec![];
    
    // Get the type being implemented
    let type_name = node.child_by_field_name("type")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
        .unwrap_or("Unknown");
    
    // Get the trait if this is a trait impl
    let trait_name = node.child_by_field_name("trait")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok());

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "function_item" {
                if let Some(mut method) = parse_function(&child, source) {
                    // Prefix method name with type for context
                    let prefix = if let Some(trait_n) = trait_name {
                        format!("{}::{}", type_name, trait_n)
                    } else {
                        type_name.to_string()
                    };
                    method.name = format!("{}::{}", prefix, method.name);
                    methods.push(method);
                }
            }
        }
    }

    if methods.is_empty() {
        None
    } else {
        Some(methods)
    }
}

/// Parse function definition
fn parse_function(node: &Node, source: &str) -> Option<Function> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let docstring = extract_outer_doc(node, source);
    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract parameters
    let mut parameters = vec![];
    if let Some(params) = node.child_by_field_name("parameters") {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            if child.kind() == "parameter" || child.kind() == "self_parameter" {
                if let Some(param) = parse_parameter(&child, source) {
                    parameters.push(param);
                }
            }
        }
    }

    // Extract return type
    let return_type = node.child_by_field_name("return_type")
        .and_then(|r| r.utf8_text(source.as_bytes()).ok())
        .map(|s| s.trim_start_matches("-> ").to_string());

    // Check for async - look for async keyword or check if source contains "async fn"
    let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");
    let is_async = node_text.starts_with("async ") || 
                   node_text.starts_with("pub async ") ||
                   node.children(&mut node.walk()).any(|c| c.kind() == "async");

    Some(Function {
        name,
        parameters,
        return_type,
        docstring,
        decorators: extract_attributes(node, source),
        is_async,
        is_generator: false,
        is_component: false,
        line_start,
        line_end,
    })
}

/// Parse function parameter
fn parse_parameter(node: &Node, source: &str) -> Option<Parameter> {
    let text = node.utf8_text(source.as_bytes()).ok()?;
    
    // Handle self parameter
    if text.contains("self") {
        return Some(Parameter {
            name: "self".to_string(),
            type_hint: if text.contains("&mut") {
                Some("&mut Self".to_string())
            } else if text.contains('&') {
                Some("&Self".to_string())
            } else {
                Some("Self".to_string())
            },
            default: None,
            kind: ParameterKind::Regular,
        });
    }

    // Regular parameter: pattern: type
    let name = node.child_by_field_name("pattern")
        .and_then(|p| p.utf8_text(source.as_bytes()).ok())
        .unwrap_or("_")
        .to_string();

    let type_hint = node.child_by_field_name("type")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    Some(Parameter {
        name,
        type_hint,
        default: None,
        kind: ParameterKind::Regular,
    })
}

/// Parse const or static item
fn parse_const(node: &Node, source: &str) -> Option<Constant> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let type_hint = node.child_by_field_name("type")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    let value = node.child_by_field_name("value")
        .and_then(|v| v.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    Some(Constant {
        name,
        type_hint,
        value,
        line: node.start_position().row + 1,
    })
}

/// Parse mod declaration
fn parse_mod(node: &Node, source: &str) -> Option<Import> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    // Only external mod declarations (no body)
    if node.child_by_field_name("body").is_some() {
        return None;
    }

    Some(Import {
        module: name,
        names: vec![],
        kind: ImportKind::Relative { level: 1 }, // mod is always relative
        line: node.start_position().row + 1,
    })
}

/// Extract outer doc comments (/// or /** */)
fn extract_outer_doc(node: &Node, source: &str) -> Option<String> {
    // Look for preceding doc comments
    if let Some(prev) = node.prev_sibling() {
        if prev.kind() == "line_comment" || prev.kind() == "block_comment" {
            let text = prev.utf8_text(source.as_bytes()).ok()?;
            if text.starts_with("///") || text.starts_with("/**") {
                let doc = text.trim_start_matches("///")
                    .trim_start_matches("/**")
                    .trim_end_matches("*/")
                    .trim();
                if !doc.is_empty() {
                    return Some(doc.to_string());
                }
            }
        }
    }
    None
}

/// Extract attributes (#[...])
fn extract_attributes(node: &Node, source: &str) -> Vec<String> {
    let mut attrs = vec![];
    let mut cursor = node.walk();
    
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_item" || child.kind() == "inner_attribute_item" {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                attrs.push(text.to_string());
            }
        }
    }
    
    attrs
}

/// Count lines in source
fn count_lines(source: &str) -> (usize, usize, usize, usize) {
    let mut total = 0;
    let mut code = 0;
    let mut comment = 0;
    let mut blank = 0;
    let mut in_block_comment = false;

    for line in source.lines() {
        total += 1;
        let trimmed = line.trim();

        if in_block_comment {
            comment += 1;
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
        } else if trimmed.is_empty() {
            blank += 1;
        } else if trimmed.starts_with("//") {
            comment += 1;
        } else if trimmed.starts_with("/*") {
            comment += 1;
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
        } else {
            code += 1;
        }
    }

    (total, code, comment, blank)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_new() {
        let parser = RustParser::new();
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_use() {
        let mut parser = RustParser::new().unwrap();
        let source = r#"
use std::collections::HashMap;
use crate::parser::ast::*;
"#;
        let file = parser.parse_source(source, "test.rs".into(), "test".into()).unwrap();
        assert_eq!(file.imports.len(), 2);
        assert_eq!(file.imports[0].module, "std::collections");
        assert_eq!(file.imports[1].module, "crate::parser::ast");
    }

    #[test]
    fn test_parse_struct() {
        let mut parser = RustParser::new().unwrap();
        let source = r#"
/// A point in 2D space
pub struct Point {
    x: f64,
    y: f64,
}
"#;
        let file = parser.parse_source(source, "test.rs".into(), "test".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        assert_eq!(file.classes[0].name, "Point");
    }

    #[test]
    fn test_parse_enum() {
        let mut parser = RustParser::new().unwrap();
        let source = r#"
pub enum Color {
    Red,
    Green,
    Blue,
}
"#;
        let file = parser.parse_source(source, "test.rs".into(), "test".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        assert_eq!(file.classes[0].name, "Color");
        assert!(file.classes[0].bases.contains(&"Enum".to_string()));
    }

    #[test]
    fn test_parse_trait() {
        let mut parser = RustParser::new().unwrap();
        let source = r#"
pub trait Drawable {
    fn draw(&self);
    fn area(&self) -> f64;
}
"#;
        let file = parser.parse_source(source, "test.rs".into(), "test".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        assert_eq!(file.classes[0].name, "Drawable");
        assert!(file.classes[0].bases.contains(&"Trait".to_string()));
        assert_eq!(file.classes[0].methods.len(), 2);
    }

    #[test]
    fn test_parse_function() {
        let mut parser = RustParser::new().unwrap();
        let source = r#"
pub fn calculate(x: i32, y: i32) -> i32 {
    x + y
}
"#;
        let file = parser.parse_source(source, "test.rs".into(), "test".into()).unwrap();
        assert_eq!(file.functions.len(), 1);
        assert_eq!(file.functions[0].name, "calculate");
        assert_eq!(file.functions[0].parameters.len(), 2);
        assert_eq!(file.functions[0].return_type, Some("i32".to_string()));
    }

    #[test]
    fn test_parse_async_function() {
        let mut parser = RustParser::new().unwrap();
        let source = r#"
pub async fn fetch_data(url: &str) -> Result<String, Error> {
    todo!()
}
"#;
        let file = parser.parse_source(source, "test.rs".into(), "test".into()).unwrap();
        assert_eq!(file.functions.len(), 1);
        assert!(file.functions[0].is_async);
    }

    #[test]
    fn test_parse_impl() {
        let mut parser = RustParser::new().unwrap();
        let source = r#"
struct Point { x: f64, y: f64 }

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    
    fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}
"#;
        let file = parser.parse_source(source, "test.rs".into(), "test".into()).unwrap();
        // Should have Point struct + 2 impl methods
        assert_eq!(file.classes.len(), 1);
        assert!(file.functions.len() >= 2);
    }

    #[test]
    fn test_parse_const() {
        let mut parser = RustParser::new().unwrap();
        let source = r#"
const MAX_SIZE: usize = 1024;
static GLOBAL: &str = "hello";
"#;
        let file = parser.parse_source(source, "test.rs".into(), "test".into()).unwrap();
        assert_eq!(file.constants.len(), 2);
        assert_eq!(file.constants[0].name, "MAX_SIZE");
        assert_eq!(file.constants[1].name, "GLOBAL");
    }

    #[test]
    fn test_path_to_module_name() {
        assert_eq!(path_to_module_name(Path::new("src/parser.rs")), "parser");
        assert_eq!(path_to_module_name(Path::new("src/parser/mod.rs")), "parser");
        assert_eq!(path_to_module_name(Path::new("src/lib.rs")), "src");
    }

    #[test]
    fn test_count_lines() {
        let source = r#"
// A comment
fn main() {
    /* block */
    println!("hello");
}
"#;
        let (total, code, comment, blank) = count_lines(source);
        assert!(total > 0);
        assert!(code > 0);
        assert!(comment >= 2);
        assert!(blank >= 1);
    }
}
