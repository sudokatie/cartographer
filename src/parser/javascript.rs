// JavaScript/TypeScript parser using tree-sitter

use crate::error::{Error, Result};
use crate::parser::ast::*;
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Supported JavaScript variants
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JsVariant {
    JavaScript,
    TypeScript,
    Jsx,
    Tsx,
}

impl JsVariant {
    /// Detect variant from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "js" => Some(Self::JavaScript),
            "jsx" => Some(Self::Jsx),
            "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "mjs" => Some(Self::JavaScript),
            "cjs" => Some(Self::JavaScript),
            "mts" => Some(Self::TypeScript),
            "cts" => Some(Self::TypeScript),
            _ => None,
        }
    }

    /// Check if this is a TypeScript variant
    pub fn is_typescript(&self) -> bool {
        matches!(self, Self::TypeScript | Self::Tsx)
    }
}

/// Parser for JavaScript/TypeScript source files
pub struct JavaScriptParser {
    js_parser: Parser,
    ts_parser: Parser,
}

impl JavaScriptParser {
    /// Create a new JavaScript/TypeScript parser
    pub fn new() -> Result<Self> {
        let mut js_parser = Parser::new();
        let js_language = tree_sitter_javascript::language();
        js_parser.set_language(&js_language).map_err(|e| {
            Error::Parser(format!("Failed to set JavaScript language: {}", e))
        })?;

        let mut ts_parser = Parser::new();
        let ts_language = tree_sitter_typescript::language_typescript();
        ts_parser.set_language(&ts_language).map_err(|e| {
            Error::Parser(format!("Failed to set TypeScript language: {}", e))
        })?;

        Ok(Self { js_parser, ts_parser })
    }

    /// Parse a JavaScript/TypeScript file
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedFile> {
        let source = std::fs::read_to_string(path).map_err(|e| {
            Error::Io(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        
        let variant = JsVariant::from_extension(ext)
            .ok_or_else(|| Error::parser(format!("Unknown JavaScript extension: {}", ext)))?;

        let module_name = path_to_module_name(path);
        self.parse_source(&source, path.to_path_buf(), module_name, variant)
    }

    /// Parse JavaScript/TypeScript source code
    pub fn parse_source(
        &mut self,
        source: &str,
        path: std::path::PathBuf,
        module_name: String,
        variant: JsVariant,
    ) -> Result<ParsedFile> {
        let parser = if variant.is_typescript() {
            &mut self.ts_parser
        } else {
            &mut self.js_parser
        };

        let tree = parser.parse(source, None).ok_or_else(|| {
            Error::parser("Failed to parse source")
        })?;

        let root = tree.root_node();
        let mut file = ParsedFile::new(path, module_name);

        // Count lines
        let (total, code, comment) = count_lines(source);
        file.total_lines = total;
        file.code_lines = code;
        file.comment_lines = comment;

        // Walk the tree and extract constructs
        self.extract_constructs(&root, source.as_bytes(), &mut file);

        Ok(file)
    }

    /// Extract all constructs from the AST
    fn extract_constructs(&self, root: &Node, source: &[u8], file: &mut ParsedFile) {
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            self.visit_node(&child, source, file);
        }
    }

    /// Visit a node and extract relevant constructs
    fn visit_node(&self, node: &Node, source: &[u8], file: &mut ParsedFile) {
        match node.kind() {
            // Import statements
            "import_statement" => {
                if let Some(import) = parse_import(node, source) {
                    file.imports.push(import);
                }
            }
            // Export statements - recurse into declarations
            "export_statement" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.visit_node(&child, source, file);
                }
            }
            // Class declarations
            "class_declaration" | "class" => {
                if let Some(class) = parse_class(node, source) {
                    file.classes.push(class);
                }
            }
            // Function declarations
            "function_declaration" | "function" => {
                if let Some(func) = parse_function(node, source) {
                    file.functions.push(func);
                }
            }
            // Arrow functions assigned to const/let/var
            "lexical_declaration" | "variable_declaration" => {
                if let Some(func) = parse_arrow_function(node, source) {
                    file.functions.push(func);
                }
            }
            _ => {
                // Recurse into children for nested declarations
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.visit_node(&child, source, file);
                }
            }
        }
    }
}

/// Convert file path to module name
fn path_to_module_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Count total, code, and comment lines
fn count_lines(source: &str) -> (usize, usize, usize) {
    let mut total = 0;
    let mut code = 0;
    let mut comment = 0;
    let mut in_block_comment = false;

    for line in source.lines() {
        total += 1;
        let trimmed = line.trim();
        
        if trimmed.is_empty() {
            continue;
        }

        if in_block_comment {
            comment += 1;
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.starts_with("//") {
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

    (total, code, comment)
}

/// Parse an import statement
fn parse_import(node: &Node, source: &[u8]) -> Option<Import> {
    let mut module = String::new();
    let mut names: Vec<ImportedName> = Vec::new();
    let mut is_relative = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string" | "string_literal" => {
                module = get_text(&child, source)
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                    .to_string();
                is_relative = module.starts_with('.');
            }
            "import_clause" => {
                let mut clause_cursor = child.walk();
                for clause_child in child.children(&mut clause_cursor) {
                    match clause_child.kind() {
                        "identifier" => {
                            // Default import
                            names.push(ImportedName::new(&get_text(&clause_child, source)));
                        }
                        "named_imports" => {
                            // Named imports: { foo, bar as baz }
                            let mut named_cursor = clause_child.walk();
                            for named_child in clause_child.children(&mut named_cursor) {
                                if named_child.kind() == "import_specifier" {
                                    if let Some(name_node) = named_child.child_by_field_name("name") {
                                        let name = get_text(&name_node, source);
                                        if let Some(alias_node) = named_child.child_by_field_name("alias") {
                                            names.push(ImportedName::with_alias(name, get_text(&alias_node, source)));
                                        } else {
                                            names.push(ImportedName::new(name));
                                        }
                                    }
                                }
                            }
                        }
                        "namespace_import" => {
                            // import * as foo
                            if let Some(name_node) = clause_child.child_by_field_name("name") {
                                names.push(ImportedName::with_alias("*", get_text(&name_node, source)));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if module.is_empty() {
        return None;
    }

    let kind = if is_relative {
        let level = module.chars().take_while(|c| *c == '.').count();
        ImportKind::Relative { level }
    } else if names.is_empty() {
        ImportKind::Direct
    } else {
        ImportKind::From
    };

    Some(Import {
        module,
        names,
        kind,
        line: node.start_position().row + 1,
    })
}

/// Parse a class declaration
fn parse_class(node: &Node, source: &[u8]) -> Option<Class> {
    let name = node.child_by_field_name("name")
        .map(|n| get_text(&n, source).to_string())?;
    
    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;
    
    let mut class = Class::new(&name, line_start);
    class.line_end = line_end;

    // Check for extends clause
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "class_heritage" {
            let mut heritage_cursor = child.walk();
            for heritage_child in child.children(&mut heritage_cursor) {
                if heritage_child.kind() == "extends_clause" {
                    if let Some(base) = heritage_child.child(1) {
                        class.bases.push(get_text(&base, source).to_string());
                    }
                }
            }
        }
    }

    // Parse class body
    if let Some(body) = node.child_by_field_name("body") {
        let mut body_cursor = body.walk();
        for child in body.children(&mut body_cursor) {
            match child.kind() {
                "method_definition" => {
                    if let Some(method) = parse_method(&child, source) {
                        class.methods.push(method);
                    }
                }
                "public_field_definition" | "field_definition" => {
                    if let Some(attr) = parse_attribute(&child, source) {
                        class.attributes.push(attr);
                    }
                }
                "comment" => {
                    // Check for JSDoc comment as class docstring
                    let text = get_text(&child, source);
                    if text.starts_with("/**") && class.docstring.is_none() {
                        class.docstring = Some(clean_jsdoc(&text));
                    }
                }
                _ => {}
            }
        }
    }

    Some(class)
}

/// Parse a method definition (returns as Function since Class uses Vec<Function>)
fn parse_method(node: &Node, source: &[u8]) -> Option<Function> {
    let name = node.child_by_field_name("name")
        .map(|n| get_text(&n, source).to_string())?;
    
    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;
    
    let mut func = Function::new(&name, line_start);
    func.line_end = line_end;
    func.parameters = parse_parameters(node, source);
    
    // Check for async/static
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "async" {
            func.is_async = true;
        }
    }

    Some(func)
}

/// Parse a field/property definition
fn parse_attribute(node: &Node, source: &[u8]) -> Option<Attribute> {
    let name = node.child_by_field_name("name")
        .map(|n| get_text(&n, source).to_string())?;
    
    Some(Attribute {
        name,
        type_hint: None,
        default: None,
        line: node.start_position().row + 1,
    })
}

/// Parse a function declaration
fn parse_function(node: &Node, source: &[u8]) -> Option<Function> {
    let name = node.child_by_field_name("name")
        .map(|n| get_text(&n, source).to_string())?;
    
    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;
    
    let mut func = Function::new(&name, line_start);
    func.line_end = line_end;
    func.parameters = parse_parameters(node, source);
    
    // Check for async keyword
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "async" {
            func.is_async = true;
        }
    }

    Some(func)
}

/// Parse an arrow function assigned to a variable
fn parse_arrow_function(node: &Node, source: &[u8]) -> Option<Function> {
    // Look for: const foo = () => {} or const foo = function() {}
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name = child.child_by_field_name("name")
                .map(|n| get_text(&n, source).to_string())?;
            
            if let Some(value) = child.child_by_field_name("value") {
                if value.kind() == "arrow_function" || value.kind() == "function" {
                    let line_start = node.start_position().row + 1;
                    let line_end = node.end_position().row + 1;
                    
                    let mut func = Function::new(&name, line_start);
                    func.line_end = line_end;
                    func.parameters = parse_parameters(&value, source);
                    
                    let mut value_cursor = value.walk();
                    for value_child in value.children(&mut value_cursor) {
                        if value_child.kind() == "async" {
                            func.is_async = true;
                        }
                    }

                    return Some(func);
                }
            }
        }
    }
    None
}

/// Parse function parameters
fn parse_parameters(node: &Node, source: &[u8]) -> Vec<Parameter> {
    let mut params = Vec::new();
    
    if let Some(params_node) = node.child_by_field_name("parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "identifier" | "required_parameter" | "optional_parameter" => {
                    let name = get_text(&child, source).to_string();
                    if !name.is_empty() && name != "(" && name != ")" && name != "," {
                        params.push(Parameter::new(&name));
                    }
                }
                "rest_pattern" => {
                    if let Some(name_node) = child.child(1) {
                        let mut param = Parameter::new(get_text(&name_node, source));
                        param.kind = ParameterKind::Args; // Rest parameter like ...args
                        params.push(param);
                    }
                }
                _ => {}
            }
        }
    }
    
    params
}

/// Get text content of a node
fn get_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.byte_range()]).unwrap_or("")
}

/// Clean JSDoc comment to extract description
fn clean_jsdoc(comment: &str) -> String {
    comment
        .trim_start_matches("/**")
        .trim_end_matches("*/")
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim())
        .filter(|line| !line.starts_with('@'))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_variant_detection() {
        assert_eq!(JsVariant::from_extension("js"), Some(JsVariant::JavaScript));
        assert_eq!(JsVariant::from_extension("jsx"), Some(JsVariant::Jsx));
        assert_eq!(JsVariant::from_extension("ts"), Some(JsVariant::TypeScript));
        assert_eq!(JsVariant::from_extension("tsx"), Some(JsVariant::Tsx));
        assert_eq!(JsVariant::from_extension("py"), None);
    }

    #[test]
    fn test_count_lines() {
        let source = "// Comment\nconst x = 1;\n/* block */\nfunction foo() {}";
        let (total, code, comment) = count_lines(source);
        assert_eq!(total, 4);
        assert_eq!(code, 2);
        assert_eq!(comment, 2);
    }

    #[test]
    fn test_parse_import() {
        let mut parser = JavaScriptParser::new().unwrap();
        let source = r#"import { foo, bar } from './module';"#;
        let result = parser.parse_source(
            source,
            std::path::PathBuf::from("test.js"),
            "test".to_string(),
            JsVariant::JavaScript,
        ).unwrap();
        
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module, "./module");
        assert!(result.imports[0].kind.is_relative());
    }

    #[test]
    fn test_parse_class() {
        let mut parser = JavaScriptParser::new().unwrap();
        let source = r#"
class MyClass {
    constructor(name) {
        this.name = name;
    }
    
    greet() {
        return `Hello, ${this.name}`;
    }
}
"#;
        let result = parser.parse_source(
            source,
            std::path::PathBuf::from("test.js"),
            "test".to_string(),
            JsVariant::JavaScript,
        ).unwrap();
        
        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].name, "MyClass");
        // Methods include constructor and greet
        assert!(result.classes[0].methods.len() >= 2);
    }

    #[test]
    fn test_parse_function() {
        let mut parser = JavaScriptParser::new().unwrap();
        let source = r#"
function greet(name) {
    return `Hello, ${name}`;
}

const farewell = (name) => {
    return `Goodbye, ${name}`;
};
"#;
        let result = parser.parse_source(
            source,
            std::path::PathBuf::from("test.js"),
            "test".to_string(),
            JsVariant::JavaScript,
        ).unwrap();
        
        assert_eq!(result.functions.len(), 2);
        assert_eq!(result.functions[0].name, "greet");
        assert_eq!(result.functions[1].name, "farewell");
    }

    #[test]
    fn test_parse_typescript() {
        let mut parser = JavaScriptParser::new().unwrap();
        let source = r#"
interface User {
    name: string;
    age: number;
}

function greet(user: User): string {
    return `Hello, ${user.name}`;
}
"#;
        let result = parser.parse_source(
            source,
            std::path::PathBuf::from("test.ts"),
            "test".to_string(),
            JsVariant::TypeScript,
        ).unwrap();
        
        // Should parse without errors
        assert!(result.functions.len() >= 1);
    }
}
