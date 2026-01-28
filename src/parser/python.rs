// Python parser using tree-sitter

use crate::error::{Error, Result};
use crate::parser::ast::*;
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Parser for Python source files
pub struct PythonParser {
    parser: Parser,
}

impl PythonParser {
    /// Create a new Python parser
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_python::language();
        parser.set_language(&language).map_err(|e| {
            Error::Parser(format!("Failed to set Python language: {}", e))
        })?;
        Ok(Self { parser })
    }

    /// Parse a Python file
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedFile> {
        let source = std::fs::read_to_string(path).map_err(|e| {
            Error::Io(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;
        
        let module_name = path_to_module_name(path);
        self.parse_source(&source, path.to_path_buf(), module_name)
    }

    /// Parse Python source code
    pub fn parse_source(
        &mut self,
        source: &str,
        path: std::path::PathBuf,
        module_name: String,
    ) -> Result<ParsedFile> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            Error::parser("Failed to parse source")
        })?;

        let root = tree.root_node();
        let mut file = ParsedFile::new(path, module_name);

        // Count lines
        let (total, code, comment) = count_lines(source);
        file.total_lines = total;
        file.code_lines = code;
        file.comment_lines = comment;

        // Extract module docstring (first expression statement that's a string)
        if let Some(docstring) = extract_module_docstring(&root, source.as_bytes()) {
            file.docstring = Some(docstring);
        }

        // Walk the tree and extract constructs
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "import_statement" => {
                    if let Some(import) = parse_import(&child, source.as_bytes()) {
                        file.imports.push(import);
                    }
                }
                "import_from_statement" => {
                    if let Some(import) = parse_import_from(&child, source.as_bytes()) {
                        file.imports.push(import);
                    }
                }
                "class_definition" => {
                    if let Some(class) = parse_class(&child, source.as_bytes()) {
                        file.classes.push(class);
                    }
                }
                "function_definition" | "decorated_definition" => {
                    if let Some(func) = parse_function(&child, source.as_bytes()) {
                        file.functions.push(func);
                    }
                }
                "expression_statement" => {
                    // Could be a constant assignment
                    if let Some(constant) = parse_constant(&child, source.as_bytes()) {
                        file.constants.push(constant);
                    }
                }
                _ => {}
            }
        }

        Ok(file)
    }
}

impl Default for PythonParser {
    fn default() -> Self {
        Self::new().expect("Failed to create parser")
    }
}

/// Convert file path to Python module name
fn path_to_module_name(path: &Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    if stem == "__init__" {
        // Use parent directory name
        path.parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| stem.to_string())
    } else {
        stem.to_string()
    }
}

/// Count lines in source: (total, code, comment)
fn count_lines(source: &str) -> (usize, usize, usize) {
    let mut total = 0;
    let mut code = 0;
    let mut comment = 0;

    for line in source.lines() {
        total += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Blank line, don't count as code
        } else if trimmed.starts_with('#') {
            comment += 1;
        } else {
            code += 1;
            // Check for inline comment
            if trimmed.contains('#') {
                comment += 1;
            }
        }
    }

    // Handle files that don't end with newline
    if !source.is_empty() && total == 0 {
        total = 1;
    }

    (total, code, comment)
}

/// Extract module docstring from root node
fn extract_module_docstring(root: &Node, source: &[u8]) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "expression_statement" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "string" {
                    return extract_string_content(&inner, source);
                }
            }
        } else if child.kind() != "comment" {
            // Stop looking after first non-comment, non-docstring statement
            break;
        }
    }
    None
}

/// Extract string content, handling triple-quoted strings
fn extract_string_content(node: &Node, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?;
    
    // Remove quotes
    let s = if text.starts_with("\"\"\"") || text.starts_with("'''") {
        &text[3..text.len().saturating_sub(3)]
    } else if text.starts_with('"') || text.starts_with('\'') {
        &text[1..text.len().saturating_sub(1)]
    } else {
        text
    };
    
    Some(s.trim().to_string())
}

/// Parse an import statement: `import x` or `import x as y`
fn parse_import(node: &Node, source: &[u8]) -> Option<Import> {
    let line = node.start_position().row + 1;
    let mut names = Vec::new();
    let mut module = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" => {
                module = child.utf8_text(source).ok()?.to_string();
                names.push(ImportedName::new(&module));
            }
            "aliased_import" => {
                let mut inner_cursor = child.walk();
                let mut name = String::new();
                let mut alias = None;
                
                for inner in child.children(&mut inner_cursor) {
                    match inner.kind() {
                        "dotted_name" => {
                            name = inner.utf8_text(source).ok()?.to_string();
                        }
                        "identifier" => {
                            alias = Some(inner.utf8_text(source).ok()?.to_string());
                        }
                        _ => {}
                    }
                }
                
                if !name.is_empty() {
                    if module.is_empty() {
                        module = name.clone();
                    }
                    if let Some(a) = alias {
                        names.push(ImportedName::with_alias(&name, &a));
                    } else {
                        names.push(ImportedName::new(&name));
                    }
                }
            }
            _ => {}
        }
    }

    if module.is_empty() {
        return None;
    }

    Some(Import {
        module,
        names,
        kind: ImportKind::Direct,
        line,
    })
}

/// Parse an import-from statement: `from x import y`
fn parse_import_from(node: &Node, source: &[u8]) -> Option<Import> {
    let line = node.start_position().row + 1;
    let mut module = String::new();
    let mut names = Vec::new();
    let mut relative_level = 0;
    let mut seen_import_keyword = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "relative_import" => {
                // Handle relative imports like `from ..utils import x`
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    match inner.kind() {
                        "import_prefix" => {
                            // Count dots
                            relative_level = inner.utf8_text(source).ok()?.chars().filter(|c| *c == '.').count();
                        }
                        "dotted_name" => {
                            module = inner.utf8_text(source).ok()?.to_string();
                        }
                        _ => {}
                    }
                }
            }
            "dotted_name" => {
                let text = child.utf8_text(source).ok()?;
                if !seen_import_keyword {
                    // This is the module being imported from
                    module = text.to_string();
                } else {
                    // This is an imported name
                    names.push(ImportedName::new(text));
                }
            }
            "import" => {
                seen_import_keyword = true;
            }
            "wildcard_import" => {
                names.push(ImportedName::new("*"));
            }
            "aliased_import" => {
                let mut inner_cursor = child.walk();
                let mut name = String::new();
                let mut alias = None;
                
                for inner in child.children(&mut inner_cursor) {
                    match inner.kind() {
                        "identifier" | "dotted_name" => {
                            if name.is_empty() {
                                name = inner.utf8_text(source).ok()?.to_string();
                            } else {
                                alias = Some(inner.utf8_text(source).ok()?.to_string());
                            }
                        }
                        _ => {}
                    }
                }
                
                if !name.is_empty() {
                    if let Some(a) = alias {
                        names.push(ImportedName::with_alias(&name, &a));
                    } else {
                        names.push(ImportedName::new(&name));
                    }
                }
            }
            _ => {}
        }
    }

    let kind = if relative_level > 0 {
        ImportKind::Relative { level: relative_level }
    } else {
        ImportKind::From
    };

    Some(Import {
        module,
        names,
        kind,
        line,
    })
}

/// Parse a class definition
fn parse_class(node: &Node, source: &[u8]) -> Option<Class> {
    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Handle decorated definitions
    let (decorators, class_node) = if node.kind() == "decorated_definition" {
        let decs = extract_decorators(node, source);
        let mut cursor = node.walk();
        let class_node = node.children(&mut cursor).find(|c| c.kind() == "class_definition")?;
        (decs, class_node)
    } else {
        (Vec::new(), *node)
    };

    let mut name = String::new();
    let mut bases = Vec::new();
    let mut docstring = None;
    let mut methods = Vec::new();
    let mut attributes = Vec::new();

    let mut cursor = class_node.walk();
    for child in class_node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if name.is_empty() {
                    name = child.utf8_text(source).ok()?.to_string();
                }
            }
            "argument_list" => {
                // Base classes
                bases = extract_bases(&child, source);
            }
            "block" => {
                // Class body
                let (doc, meths, attrs) = parse_class_body(&child, source);
                docstring = doc;
                methods = meths;
                attributes = attrs;
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Class {
        name,
        docstring,
        bases,
        decorators,
        methods,
        attributes,
        line_start,
        line_end,
    })
}

/// Extract base classes from argument list
fn extract_bases(node: &Node, source: &[u8]) -> Vec<String> {
    let mut bases = Vec::new();
    let mut cursor = node.walk();
    
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" | "attribute" => {
                if let Ok(text) = child.utf8_text(source) {
                    bases.push(text.to_string());
                }
            }
            "call" => {
                // Handle things like Generic[T]
                if let Ok(text) = child.utf8_text(source) {
                    bases.push(text.to_string());
                }
            }
            _ => {}
        }
    }
    
    bases
}

/// Parse class body to extract docstring, methods, and attributes
fn parse_class_body(node: &Node, source: &[u8]) -> (Option<String>, Vec<Function>, Vec<Attribute>) {
    let mut docstring = None;
    let mut methods = Vec::new();
    let mut attributes = Vec::new();
    let mut first_statement = true;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "expression_statement" => {
                // Check for docstring (first string expression)
                if first_statement {
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "string" {
                            docstring = extract_string_content(&inner, source);
                            break;
                        }
                    }
                }
                first_statement = false;
                
                // Check for class attribute assignment
                if let Some(attr) = parse_class_attribute(&child, source) {
                    attributes.push(attr);
                }
            }
            "function_definition" | "decorated_definition" => {
                first_statement = false;
                if let Some(method) = parse_function(&child, source) {
                    methods.push(method);
                }
            }
            _ => {
                first_statement = false;
            }
        }
    }

    (docstring, methods, attributes)
}

/// Parse a class attribute assignment
fn parse_class_attribute(node: &Node, source: &[u8]) -> Option<Attribute> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "assignment" {
            let line = child.start_position().row + 1;
            let mut inner_cursor = child.walk();
            
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "identifier" {
                    let name = inner.utf8_text(source).ok()?;
                    // Skip if it looks like a method call result or private
                    if !name.starts_with('_') {
                        return Some(Attribute::new(name, line));
                    }
                } else if inner.kind() == "type" {
                    // Type annotation
                }
            }
        }
    }
    None
}

/// Parse a function definition
fn parse_function(node: &Node, source: &[u8]) -> Option<Function> {
    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Handle decorated definitions
    if node.kind() == "decorated_definition" {
        let decs = extract_decorators(node, source);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "function_definition" {
                // Check for async keyword inside function_definition
                let is_async = has_async_keyword(&child);
                return parse_function_node(&child, source, decs, is_async, line_start, line_end);
            }
        }
        return None;
    }

    // Check for async keyword inside function_definition
    let is_async = has_async_keyword(node);
    parse_function_node(node, source, Vec::new(), is_async, line_start, line_end)
}

/// Check if a function_definition node has an async keyword
fn has_async_keyword(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "async" {
            return true;
        }
    }
    false
}

fn parse_function_node(
    node: &Node,
    source: &[u8],
    decorators: Vec<String>,
    is_async: bool,
    line_start: usize,
    line_end: usize,
) -> Option<Function> {
    let mut name = String::new();
    let mut parameters = Vec::new();
    let mut return_type = None;
    let mut docstring = None;
    let mut is_generator = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" | "name" => {
                if name.is_empty() {
                    name = child.utf8_text(source).ok()?.to_string();
                }
            }
            "parameters" => {
                parameters = parse_parameters(&child, source);
            }
            "type" => {
                return_type = Some(child.utf8_text(source).ok()?.to_string());
            }
            "block" => {
                // Extract docstring and check for yield
                let mut inner_cursor = child.walk();
                let mut first = true;
                for inner in child.children(&mut inner_cursor) {
                    if first && inner.kind() == "expression_statement" {
                        let mut expr_cursor = inner.walk();
                        for expr in inner.children(&mut expr_cursor) {
                            if expr.kind() == "string" {
                                docstring = extract_string_content(&expr, source);
                            }
                        }
                    }
                    first = false;
                    
                    // Check for yield (basic check)
                    if inner.kind() == "yield" || inner.kind() == "yield_statement" {
                        is_generator = true;
                    }
                }
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Function {
        name,
        docstring,
        parameters,
        return_type,
        decorators,
        is_async,
        is_generator,
        line_start,
        line_end,
    })
}

/// Extract decorators from a decorated definition
fn extract_decorators(node: &Node, source: &[u8]) -> Vec<String> {
    let mut decorators = Vec::new();
    let mut cursor = node.walk();
    
    for child in node.children(&mut cursor) {
        if child.kind() == "decorator" {
            if let Ok(text) = child.utf8_text(source) {
                // Remove @ prefix and any arguments
                let dec = text.trim_start_matches('@');
                let dec = if let Some(idx) = dec.find('(') {
                    &dec[..idx]
                } else {
                    dec
                };
                decorators.push(dec.trim().to_string());
            }
        }
    }
    
    decorators
}

/// Parse function parameters
fn parse_parameters(node: &Node, source: &[u8]) -> Vec<Parameter> {
    let mut params = Vec::new();
    let mut cursor = node.walk();
    let mut seen_star = false;
    let mut seen_slash = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                let name = child.utf8_text(source).unwrap_or("?").to_string();
                let kind = if seen_star {
                    ParameterKind::KeywordOnly
                } else if !seen_slash {
                    ParameterKind::PositionalOnly
                } else {
                    ParameterKind::Regular
                };
                let mut param = Parameter::new(&name);
                param.kind = kind;
                params.push(param);
            }
            "typed_parameter" => {
                if let Some(param) = parse_typed_parameter(&child, source, seen_star) {
                    params.push(param);
                }
            }
            "default_parameter" => {
                if let Some(param) = parse_default_parameter(&child, source, seen_star) {
                    params.push(param);
                }
            }
            "typed_default_parameter" => {
                if let Some(param) = parse_typed_default_parameter(&child, source, seen_star) {
                    params.push(param);
                }
            }
            "list_splat_pattern" | "dictionary_splat_pattern" => {
                // *args or **kwargs
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "identifier" {
                        let name = inner.utf8_text(source).unwrap_or("?");
                        let mut param = Parameter::new(name);
                        param.kind = if child.kind() == "list_splat_pattern" {
                            seen_star = true;
                            ParameterKind::Args
                        } else {
                            ParameterKind::Kwargs
                        };
                        params.push(param);
                    }
                }
            }
            "*" => {
                seen_star = true;
            }
            "/" => {
                seen_slash = true;
            }
            _ => {}
        }
    }

    params
}

fn parse_typed_parameter(node: &Node, source: &[u8], keyword_only: bool) -> Option<Parameter> {
    let mut name = String::new();
    let mut type_hint = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if name.is_empty() {
                    name = child.utf8_text(source).ok()?.to_string();
                }
            }
            "type" => {
                type_hint = Some(child.utf8_text(source).ok()?.to_string());
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    let mut param = Parameter::new(&name);
    param.type_hint = type_hint;
    if keyword_only {
        param.kind = ParameterKind::KeywordOnly;
    }
    Some(param)
}

fn parse_default_parameter(node: &Node, source: &[u8], keyword_only: bool) -> Option<Parameter> {
    let mut name = String::new();
    let mut default = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if name.is_empty() {
                    name = child.utf8_text(source).ok()?.to_string();
                }
            }
            _ => {
                // Assume anything else is the default value
                if !name.is_empty() && default.is_none() && child.kind() != "=" {
                    default = Some(child.utf8_text(source).ok()?.to_string());
                }
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    let mut param = Parameter::new(&name);
    param.default = default;
    if keyword_only {
        param.kind = ParameterKind::KeywordOnly;
    }
    Some(param)
}

fn parse_typed_default_parameter(node: &Node, source: &[u8], keyword_only: bool) -> Option<Parameter> {
    let mut name = String::new();
    let mut type_hint = None;
    let mut default = None;
    let mut cursor = node.walk();
    let mut past_equals = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if name.is_empty() {
                    name = child.utf8_text(source).ok()?.to_string();
                }
            }
            "type" => {
                type_hint = Some(child.utf8_text(source).ok()?.to_string());
            }
            ":" => {
                // Type annotation follows
            }
            "=" => {
                past_equals = true;
            }
            _ => {
                if past_equals && default.is_none() {
                    default = Some(child.utf8_text(source).ok()?.to_string());
                }
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    let mut param = Parameter::new(&name);
    param.type_hint = type_hint;
    param.default = default;
    if keyword_only {
        param.kind = ParameterKind::KeywordOnly;
    }
    Some(param)
}

/// Parse a potential constant assignment
fn parse_constant(node: &Node, source: &[u8]) -> Option<Constant> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "assignment" {
            let line = child.start_position().row + 1;
            let mut inner_cursor = child.walk();
            let mut name = None;
            let mut value = None;
            let mut type_hint = None;

            for inner in child.children(&mut inner_cursor) {
                match inner.kind() {
                    "identifier" => {
                        let n = inner.utf8_text(source).ok()?;
                        // Only treat ALL_CAPS as constants
                        if n.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit()) {
                            name = Some(n.to_string());
                        }
                    }
                    "type" => {
                        type_hint = Some(inner.utf8_text(source).ok()?.to_string());
                    }
                    "=" => {}
                    _ => {
                        if name.is_some() && value.is_none() {
                            value = Some(inner.utf8_text(source).ok()?.to_string());
                        }
                    }
                }
            }

            if let Some(n) = name {
                return Some(Constant {
                    name: n,
                    type_hint,
                    value,
                    line,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse(source: &str) -> ParsedFile {
        let mut parser = PythonParser::new().unwrap();
        parser.parse_source(source, PathBuf::from("test.py"), "test".to_string()).unwrap()
    }

    #[test]
    fn test_parser_new() {
        let parser = PythonParser::new();
        assert!(parser.is_ok());
    }

    #[test]
    fn test_empty_file() {
        let file = parse("");
        assert!(file.is_empty());
    }

    #[test]
    fn test_module_docstring() {
        let file = parse(r#""""Module docstring.""""#);
        assert_eq!(file.docstring, Some("Module docstring.".to_string()));
    }

    #[test]
    fn test_simple_import() {
        let file = parse("import os");
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.imports[0].module, "os");
        assert_eq!(file.imports[0].kind, ImportKind::Direct);
    }

    #[test]
    fn test_import_with_alias() {
        let file = parse("import numpy as np");
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.imports[0].names.len(), 1);
        assert_eq!(file.imports[0].names[0].used_name(), "np");
    }

    #[test]
    fn test_from_import() {
        let file = parse("from os import path, getcwd");
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.imports[0].module, "os");
        assert_eq!(file.imports[0].kind, ImportKind::From);
        assert!(file.imports[0].names.len() >= 1);
    }

    #[test]
    fn test_relative_import() {
        let file = parse("from ..utils import helper");
        assert_eq!(file.imports.len(), 1);
        if let ImportKind::Relative { level } = file.imports[0].kind {
            assert_eq!(level, 2);
        } else {
            panic!("Expected relative import");
        }
    }

    #[test]
    fn test_simple_function() {
        let file = parse("def hello(): pass");
        assert_eq!(file.functions.len(), 1);
        assert_eq!(file.functions[0].name, "hello");
    }

    #[test]
    fn test_function_with_params() {
        let file = parse("def greet(name: str, age: int = 0) -> str: pass");
        assert_eq!(file.functions.len(), 1);
        let func = &file.functions[0];
        assert_eq!(func.name, "greet");
        assert_eq!(func.parameters.len(), 2);
        assert_eq!(func.parameters[0].name, "name");
        assert_eq!(func.parameters[0].type_hint, Some("str".to_string()));
        assert_eq!(func.return_type, Some("str".to_string()));
    }

    #[test]
    fn test_async_function() {
        let file = parse("async def fetch(url): pass");
        assert_eq!(file.functions.len(), 1);
        assert!(file.functions[0].is_async);
    }

    #[test]
    fn test_decorated_function() {
        let file = parse("@staticmethod\ndef helper(): pass");
        assert_eq!(file.functions.len(), 1);
        assert!(file.functions[0].is_staticmethod());
    }

    #[test]
    fn test_simple_class() {
        let file = parse("class MyClass: pass");
        assert_eq!(file.classes.len(), 1);
        assert_eq!(file.classes[0].name, "MyClass");
    }

    #[test]
    fn test_class_with_bases() {
        let file = parse("class MyClass(Base, Mixin): pass");
        assert_eq!(file.classes.len(), 1);
        assert!(file.classes[0].bases.len() >= 1);
    }

    #[test]
    fn test_class_with_method() {
        let file = parse("class MyClass:\n    def method(self): pass");
        assert_eq!(file.classes.len(), 1);
        assert_eq!(file.classes[0].methods.len(), 1);
        assert_eq!(file.classes[0].methods[0].name, "method");
    }

    #[test]
    fn test_constant() {
        let file = parse("MAX_SIZE = 100");
        assert_eq!(file.constants.len(), 1);
        assert_eq!(file.constants[0].name, "MAX_SIZE");
    }

    #[test]
    fn test_line_counting() {
        let source = "# Comment\n\ndef func():\n    pass\n";
        let (total, code, comment) = count_lines(source);
        assert_eq!(total, 4);
        assert_eq!(code, 2);
        assert_eq!(comment, 1);
    }

    #[test]
    fn test_path_to_module_name() {
        assert_eq!(path_to_module_name(Path::new("test.py")), "test");
        assert_eq!(path_to_module_name(Path::new("pkg/__init__.py")), "pkg");
    }
}
