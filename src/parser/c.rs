// C parser using tree-sitter

use crate::error::{Error, Result};
use crate::parser::ast::{
    Attribute, Class, Constant, Function, Import, ImportKind, Parameter, ParameterKind, ParsedFile,
};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Parser for C source files
pub struct CParser {
    parser: Parser,
}

impl CParser {
    /// Create a new C parser
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_c::language();
        parser.set_language(&language).map_err(|e| {
            Error::Parser(format!("Failed to set C language: {}", e))
        })?;

        Ok(Self { parser })
    }

    /// Parse a C file
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedFile> {
        let source = std::fs::read_to_string(path).map_err(|e| {
            Error::Io(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        let module_name = path_to_module_name(path);
        self.parse_source(&source, path.to_path_buf(), module_name)
    }

    /// Parse C source code
    pub fn parse_source(
        &mut self,
        source: &str,
        path: std::path::PathBuf,
        module_name: String,
    ) -> Result<ParsedFile> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            Error::parser("Failed to parse C source")
        })?;

        let root = tree.root_node();
        let mut file = ParsedFile::new(path, module_name);

        // Walk the tree and extract constructs
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "preproc_include" => {
                    if let Some(import) = parse_include(&child, source) {
                        file.imports.push(import);
                    }
                }
                "function_definition" => {
                    if let Some(func) = parse_function(&child, source) {
                        file.functions.push(func);
                    }
                }
                "declaration" => {
                    // Could be function declaration, struct, enum, typedef, or variable
                    parse_declaration(&child, source, &mut file);
                }
                "struct_specifier" | "enum_specifier" => {
                    if let Some(class) = parse_type_specifier(&child, source) {
                        file.classes.push(class);
                    }
                }
                "type_definition" => {
                    // typedef - extract the new type name
                    if let Some(class) = parse_typedef(&child, source) {
                        file.classes.push(class);
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

impl Default for CParser {
    fn default() -> Self {
        Self::new().expect("Failed to create CParser")
    }
}

/// Convert file path to module name
fn path_to_module_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Parse #include directive
fn parse_include(node: &Node, source: &str) -> Option<Import> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string_literal" || child.kind() == "system_lib_string" {
            let text = child.utf8_text(source.as_bytes()).ok()?;
            // Remove quotes or angle brackets
            let module = text
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_string();
            
            return Some(Import {
                module,
                names: vec![],
                kind: ImportKind::Direct,
                line: node.start_position().row + 1,
            });
        }
    }
    None
}

/// Parse function definition
fn parse_function(node: &Node, source: &str) -> Option<Function> {
    // Get declarator which contains the function name and parameters
    let declarator = node.child_by_field_name("declarator")?;
    
    let (name, parameters) = parse_function_declarator(&declarator, source)?;
    
    // Get return type
    let return_type = node.child_by_field_name("type")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    let docstring = extract_comment(node, source);

    Some(Function {
        name,
        parameters,
        return_type,
        decorators: vec![],
        is_async: false,
        is_generator: false,
        is_component: false,
        docstring,
        line_start,
        line_end,
    })
}

/// Parse function declarator to get name and parameters
fn parse_function_declarator(node: &Node, source: &str) -> Option<(String, Vec<Parameter>)> {
    let mut name = None;
    let mut parameters = vec![];

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                name = child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
            }
            "function_declarator" => {
                // Nested function declarator (for pointers to functions)
                return parse_function_declarator(&child, source);
            }
            "parameter_list" => {
                parameters = parse_parameters(&child, source);
            }
            "pointer_declarator" => {
                // Handle pointer to function
                let mut ptr_cursor = child.walk();
                for ptr_child in child.children(&mut ptr_cursor) {
                    if ptr_child.kind() == "function_declarator" {
                        return parse_function_declarator(&ptr_child, source);
                    } else if ptr_child.kind() == "identifier" {
                        name = ptr_child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    name.map(|n| (n, parameters))
}

/// Parse parameter list
fn parse_parameters(node: &Node, source: &str) -> Vec<Parameter> {
    let mut parameters = vec![];
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "parameter_declaration" => {
                if let Some(param) = parse_parameter(&child, source) {
                    parameters.push(param);
                }
            }
            "variadic_parameter" => {
                parameters.push(Parameter {
                    name: "...".to_string(),
                    type_hint: Some("...".to_string()),
                    default: None,
                    kind: ParameterKind::Args,
                });
            }
            _ => {}
        }
    }

    parameters
}

/// Parse a single parameter declaration
fn parse_parameter(node: &Node, source: &str) -> Option<Parameter> {
    let type_hint = node.child_by_field_name("type")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    // The declarator contains the parameter name
    let name = node.child_by_field_name("declarator")
        .and_then(|d| extract_identifier(&d, source))
        .unwrap_or_else(|| "param".to_string());

    Some(Parameter {
        name,
        type_hint,
        default: None,
        kind: ParameterKind::Regular,
    })
}

/// Extract identifier from a declarator (handles pointers, arrays)
fn extract_identifier(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" => node.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()),
        "pointer_declarator" | "array_declarator" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(id) = extract_identifier(&child, source) {
                    return Some(id);
                }
            }
            None
        }
        _ => None,
    }
}

/// Parse declarations (variables, function prototypes, etc.)
fn parse_declaration(node: &Node, source: &str, file: &mut ParsedFile) {
    // Check if it's a function declaration (prototype)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declarator" => {
                // This is a function prototype
                if let Some((name, parameters)) = parse_function_declarator(&child, source) {
                    let return_type = node.child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());

                    let line = node.start_position().row + 1;
                    file.functions.push(Function {
                        name,
                        parameters,
                        return_type,
                        decorators: vec!["prototype".to_string()],
                        is_async: false,
                        is_generator: false,
                        is_component: false,
                        docstring: extract_comment(node, source),
                        line_start: line,
                        line_end: line,
                    });
                }
                return;
            }
            "struct_specifier" | "enum_specifier" => {
                if let Some(class) = parse_type_specifier(&child, source) {
                    file.classes.push(class);
                }
            }
            "init_declarator" => {
                // Variable declaration with optional initializer
                if let Some(constant) = parse_init_declarator(&child, node, source) {
                    file.constants.push(constant);
                }
            }
            "identifier" | "pointer_declarator" | "array_declarator" => {
                // Simple variable declaration
                if let Some(name) = extract_identifier(&child, source) {
                    let type_hint = node.child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());

                    file.constants.push(Constant {
                        name,
                        type_hint,
                        value: None,
                        line: node.start_position().row + 1,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Parse init_declarator (variable with initializer)
fn parse_init_declarator(node: &Node, decl_node: &Node, source: &str) -> Option<Constant> {
    let declarator = node.child_by_field_name("declarator")?;
    let name = extract_identifier(&declarator, source)?;

    let type_hint = decl_node.child_by_field_name("type")
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

/// Parse struct or enum specifier
fn parse_type_specifier(node: &Node, source: &str) -> Option<Class> {
    let kind = node.kind();
    let name = node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())?;

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    let mut attributes = vec![];

    // Parse body for struct fields or enum values
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "field_declaration" => {
                    // Struct field
                    let attrs = parse_field_declaration(&child, source);
                    attributes.extend(attrs);
                }
                "enumerator" => {
                    // Enum value
                    if let Some(attr) = parse_enumerator(&child, source) {
                        attributes.push(attr);
                    }
                }
                _ => {}
            }
        }
    }

    let bases = match kind {
        "struct_specifier" => vec!["struct".to_string()],
        "enum_specifier" => vec!["enum".to_string()],
        _ => vec![],
    };

    let docstring = extract_comment(node, source);

    Some(Class {
        name,
        bases,
        methods: vec![],
        decorators: vec![],
        docstring,
        line_start,
        line_end,
        attributes,
    })
}

/// Parse struct field declaration
fn parse_field_declaration(node: &Node, source: &str) -> Vec<Attribute> {
    let mut attributes = vec![];

    let type_hint = node.child_by_field_name("type")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "field_identifier" || child.kind() == "pointer_declarator" || child.kind() == "array_declarator" {
            if let Some(name) = extract_field_name(&child, source) {
                attributes.push(Attribute {
                    name,
                    type_hint: type_hint.clone(),
                    default: None,
                    line: child.start_position().row + 1,
                });
            }
        }
    }

    attributes
}

/// Extract field name from declarator
fn extract_field_name(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "field_identifier" => node.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()),
        "pointer_declarator" | "array_declarator" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(name) = extract_field_name(&child, source) {
                    return Some(name);
                }
            }
            None
        }
        _ => None,
    }
}

/// Parse enum value
fn parse_enumerator(node: &Node, source: &str) -> Option<Attribute> {
    let name = node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())?;

    let value = node.child_by_field_name("value")
        .and_then(|v| v.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    Some(Attribute {
        name,
        type_hint: None,
        default: value,
        line: node.start_position().row + 1,
    })
}

/// Parse typedef
fn parse_typedef(node: &Node, source: &str) -> Option<Class> {
    // Get the type being defined
    let mut type_name = None;
    let mut underlying = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" => {
                type_name = child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
            }
            "struct_specifier" | "enum_specifier" => {
                // typedef struct { ... } Name;
                if let Some(class) = parse_type_specifier(&child, source) {
                    // The actual typedef name comes later
                    underlying = Some(class);
                }
            }
            "primitive_type" | "sized_type_specifier" => {
                underlying = Some(Class {
                    name: String::new(),
                    bases: vec!["typedef".to_string()],
                    methods: vec![],
                    decorators: vec![],
                    docstring: None,
                    line_start: node.start_position().row + 1,
                    line_end: node.end_position().row + 1,
                    attributes: vec![],
                });
            }
            _ => {}
        }
    }

    if let Some(name) = type_name {
        if let Some(mut class) = underlying {
            if class.name.is_empty() {
                class.name = name;
            } else {
                // Named struct/enum typedef - use the typedef name
                class.name = name;
            }
            class.bases.push("typedef".to_string());
            return Some(class);
        }
        // Simple typedef
        return Some(Class {
            name,
            bases: vec!["typedef".to_string()],
            methods: vec![],
            decorators: vec![],
            docstring: extract_comment(node, source),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
            attributes: vec![],
        });
    }

    None
}

/// Extract comment preceding a node
fn extract_comment(node: &Node, source: &str) -> Option<String> {
    if let Some(prev) = node.prev_sibling() {
        if prev.kind() == "comment" {
            let text = prev.utf8_text(source.as_bytes()).ok()?;
            let doc = text
                .trim_start_matches("//")
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim();
            if !doc.is_empty() {
                return Some(doc.to_string());
            }
        }
    }
    None
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
        let parser = CParser::new();
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_includes() {
        let mut parser = CParser::new().unwrap();
        let source = r#"
#include <stdio.h>
#include <stdlib.h>
#include "myheader.h"

int main() { return 0; }
"#;
        let file = parser.parse_source(source, "main.c".into(), "main".into()).unwrap();
        assert_eq!(file.imports.len(), 3);
        assert_eq!(file.imports[0].module, "stdio.h");
        assert_eq!(file.imports[1].module, "stdlib.h");
        assert_eq!(file.imports[2].module, "myheader.h");
    }

    #[test]
    fn test_parse_function() {
        let mut parser = CParser::new().unwrap();
        let source = r#"
int add(int a, int b) {
    return a + b;
}
"#;
        let file = parser.parse_source(source, "math.c".into(), "math".into()).unwrap();
        assert_eq!(file.functions.len(), 1);
        
        let func = &file.functions[0];
        assert_eq!(func.name, "add");
        assert_eq!(func.parameters.len(), 2);
        assert_eq!(func.return_type, Some("int".to_string()));
    }

    #[test]
    fn test_parse_function_prototype() {
        let mut parser = CParser::new().unwrap();
        let source = r#"
int add(int a, int b);
void process(char *data, size_t len);
"#;
        let file = parser.parse_source(source, "header.h".into(), "header".into()).unwrap();
        assert_eq!(file.functions.len(), 2);
        assert!(file.functions[0].decorators.contains(&"prototype".to_string()));
    }

    #[test]
    fn test_parse_struct() {
        let mut parser = CParser::new().unwrap();
        let source = r#"
struct Point {
    int x;
    int y;
};
"#;
        let file = parser.parse_source(source, "types.c".into(), "types".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let class = &file.classes[0];
        assert_eq!(class.name, "Point");
        assert!(class.bases.contains(&"struct".to_string()));
        assert_eq!(class.attributes.len(), 2);
    }

    #[test]
    fn test_parse_enum() {
        let mut parser = CParser::new().unwrap();
        let source = r#"
enum Color {
    RED,
    GREEN = 5,
    BLUE
};
"#;
        let file = parser.parse_source(source, "types.c".into(), "types".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let class = &file.classes[0];
        assert_eq!(class.name, "Color");
        assert!(class.bases.contains(&"enum".to_string()));
        assert_eq!(class.attributes.len(), 3);
        assert_eq!(class.attributes[1].default, Some("5".to_string()));
    }

    #[test]
    fn test_parse_typedef() {
        let mut parser = CParser::new().unwrap();
        let source = r#"
typedef unsigned int uint;
typedef struct {
    int x;
    int y;
} Point;
"#;
        let file = parser.parse_source(source, "types.c".into(), "types".into()).unwrap();
        assert!(file.classes.len() >= 1);
    }

    #[test]
    fn test_parse_variadic_function() {
        let mut parser = CParser::new().unwrap();
        let source = r#"
int printf(const char *format, ...);
"#;
        let file = parser.parse_source(source, "stdio.h".into(), "stdio".into()).unwrap();
        assert_eq!(file.functions.len(), 1);
        
        let func = &file.functions[0];
        assert_eq!(func.parameters.len(), 2);
        assert_eq!(func.parameters[1].kind, ParameterKind::Args);
    }

    #[test]
    fn test_parse_global_variables() {
        let mut parser = CParser::new().unwrap();
        let source = r#"
int global_count = 0;
const char *message = "hello";
"#;
        let file = parser.parse_source(source, "globals.c".into(), "globals".into()).unwrap();
        assert!(file.constants.len() >= 1);
    }

    #[test]
    fn test_count_lines() {
        let source = r#"
#include <stdio.h>

// Single line comment
int main() {
    /* Block comment */
    printf("Hello\n");
    return 0;
}
"#;
        let (total, code, comment, blank) = count_lines(source);
        assert!(total > 0);
        assert!(code > 0);
        assert!(comment >= 2);
        assert!(blank >= 1);
    }
}
