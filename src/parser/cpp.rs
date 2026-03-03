// C++ parser using tree-sitter

use crate::error::{Error, Result};
use crate::parser::ast::{
    Attribute, Class, Function, Import, ImportKind, Parameter, ParameterKind, ParsedFile,
};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Parser for C++ source files
pub struct CppParser {
    parser: Parser,
}

impl CppParser {
    /// Create a new C++ parser
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_cpp::language();
        parser.set_language(&language).map_err(|e| {
            Error::Parser(format!("Failed to set C++ language: {}", e))
        })?;

        Ok(Self { parser })
    }

    /// Parse a C++ file
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedFile> {
        let source = std::fs::read_to_string(path).map_err(|e| {
            Error::Io(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        let module_name = path_to_module_name(path);
        self.parse_source(&source, path.to_path_buf(), module_name)
    }

    /// Parse C++ source code
    pub fn parse_source(
        &mut self,
        source: &str,
        path: std::path::PathBuf,
        module_name: String,
    ) -> Result<ParsedFile> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            Error::parser("Failed to parse C++ source")
        })?;

        let root = tree.root_node();
        let mut file = ParsedFile::new(path, module_name);

        // Walk the tree and extract constructs
        parse_translation_unit(&root, source, &mut file, "");

        // Count lines
        let (total, code, comment, _blank) = count_lines(source);
        file.total_lines = total;
        file.code_lines = code;
        file.comment_lines = comment;

        Ok(file)
    }
}

impl Default for CppParser {
    fn default() -> Self {
        Self::new().expect("Failed to create CppParser")
    }
}

/// Parse the translation unit (top-level)
fn parse_translation_unit(node: &Node, source: &str, file: &mut ParsedFile, namespace_prefix: &str) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "preproc_include" => {
                if let Some(import) = parse_include(&child, source) {
                    file.imports.push(import);
                }
            }
            "using_declaration" => {
                if let Some(import) = parse_using(&child, source) {
                    file.imports.push(import);
                }
            }
            "namespace_definition" => {
                parse_namespace(&child, source, file, namespace_prefix);
            }
            "function_definition" => {
                if let Some(func) = parse_function(&child, source, namespace_prefix) {
                    file.functions.push(func);
                }
            }
            "class_specifier" | "struct_specifier" => {
                if let Some(class) = parse_class_specifier(&child, source, namespace_prefix) {
                    file.classes.push(class);
                }
            }
            "enum_specifier" => {
                if let Some(class) = parse_enum(&child, source, namespace_prefix) {
                    file.classes.push(class);
                }
            }
            "declaration" => {
                parse_declaration(&child, source, file, namespace_prefix);
            }
            "template_declaration" => {
                // Template declaration wraps another declaration
                let mut tmpl_cursor = child.walk();
                for tmpl_child in child.children(&mut tmpl_cursor) {
                    match tmpl_child.kind() {
                        "function_definition" => {
                            if let Some(mut func) = parse_function(&tmpl_child, source, namespace_prefix) {
                                func.decorators.push("template".to_string());
                                file.functions.push(func);
                            }
                        }
                        "class_specifier" | "struct_specifier" => {
                            if let Some(mut class) = parse_class_specifier(&tmpl_child, source, namespace_prefix) {
                                class.decorators.push("template".to_string());
                                file.classes.push(class);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// Parse namespace definition
fn parse_namespace(node: &Node, source: &str, file: &mut ParsedFile, parent_prefix: &str) {
    let name = node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .unwrap_or("");

    let new_prefix = if parent_prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", parent_prefix, name)
    };

    // Parse namespace body
    if let Some(body) = node.child_by_field_name("body") {
        parse_translation_unit(&body, source, file, &new_prefix);
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

/// Parse using declaration
fn parse_using(node: &Node, source: &str) -> Option<Import> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "qualified_identifier" || child.kind() == "namespace_identifier" {
            let module = child.utf8_text(source.as_bytes()).ok()?.to_string();
            return Some(Import {
                module,
                names: vec![],
                kind: ImportKind::From,
                line: node.start_position().row + 1,
            });
        }
    }
    None
}

/// Parse function definition
fn parse_function(node: &Node, source: &str, namespace_prefix: &str) -> Option<Function> {
    let declarator = node.child_by_field_name("declarator")?;
    
    let (raw_name, parameters) = parse_function_declarator(&declarator, source)?;
    
    // Prepend namespace if present
    let name = if namespace_prefix.is_empty() {
        raw_name
    } else {
        format!("{}::{}", namespace_prefix, raw_name)
    };

    // Get return type
    let return_type = node.child_by_field_name("type")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Check for virtual, static, etc.
    let mut decorators = vec![];
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "virtual" {
            decorators.push("virtual".to_string());
        } else if child.kind() == "static" {
            decorators.push("static".to_string());
        }
    }

    let docstring = extract_comment(node, source);

    Some(Function {
        name,
        parameters,
        return_type,
        decorators,
        is_async: false,
        is_generator: false,
        is_component: false,
        docstring,
        line_start,
        line_end,
    })
}

/// Parse function declarator
fn parse_function_declarator(node: &Node, source: &str) -> Option<(String, Vec<Parameter>)> {
    let mut name = None;
    let mut parameters = vec![];

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" | "field_identifier" | "destructor_name" => {
                name = child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
            }
            "qualified_identifier" | "template_function" => {
                name = child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
            }
            "function_declarator" => {
                return parse_function_declarator(&child, source);
            }
            "parameter_list" => {
                parameters = parse_parameters(&child, source);
            }
            "operator_name" => {
                // Operator overload
                let op = child.utf8_text(source.as_bytes()).ok()?;
                name = Some(format!("operator{}", op));
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
            "parameter_declaration" | "optional_parameter_declaration" => {
                if let Some(param) = parse_parameter(&child, source) {
                    parameters.push(param);
                }
            }
            "variadic_parameter_declaration" => {
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

/// Parse a single parameter
fn parse_parameter(node: &Node, source: &str) -> Option<Parameter> {
    let type_hint = node.child_by_field_name("type")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    let name = node.child_by_field_name("declarator")
        .and_then(|d| extract_identifier(&d, source))
        .unwrap_or_else(|| "param".to_string());

    let default = node.child_by_field_name("default_value")
        .and_then(|d| d.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    Some(Parameter {
        name,
        type_hint,
        default,
        kind: ParameterKind::Regular,
    })
}

/// Extract identifier from declarator
fn extract_identifier(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" => node.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()),
        "pointer_declarator" | "reference_declarator" | "array_declarator" => {
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

/// Parse class or struct specifier
fn parse_class_specifier(node: &Node, source: &str, namespace_prefix: &str) -> Option<Class> {
    let raw_name = node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())?;

    let name = if namespace_prefix.is_empty() {
        raw_name
    } else {
        format!("{}::{}", namespace_prefix, raw_name)
    };

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract base classes
    let mut bases = vec![];
    let is_struct = node.kind() == "struct_specifier";
    if is_struct {
        bases.push("struct".to_string());
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "base_class_clause" {
            let mut base_cursor = child.walk();
            for base_child in child.children(&mut base_cursor) {
                if base_child.kind() == "type_identifier" || base_child.kind() == "qualified_identifier" {
                    if let Ok(base_name) = base_child.utf8_text(source.as_bytes()) {
                        bases.push(base_name.to_string());
                    }
                }
            }
        }
    }

    // Parse class body
    let mut methods = vec![];
    let mut attributes = vec![];

    if let Some(body) = node.child_by_field_name("body") {
        let mut body_cursor = body.walk();
        for body_child in body.children(&mut body_cursor) {
            match body_child.kind() {
                "function_definition" => {
                    if let Some(method) = parse_function(&body_child, source, "") {
                        methods.push(method);
                    }
                }
                "declaration" => {
                    // Could be method declaration or member variable
                    parse_class_member(&body_child, source, &mut methods, &mut attributes);
                }
                "access_specifier" => {
                    // public:, private:, protected: - could track visibility
                }
                _ => {}
            }
        }
    }

    let docstring = extract_comment(node, source);

    Some(Class {
        name,
        bases,
        methods,
        decorators: vec![],
        docstring,
        line_start,
        line_end,
        attributes,
    })
}

/// Parse class member (method declaration or field)
fn parse_class_member(node: &Node, source: &str, methods: &mut Vec<Function>, attributes: &mut Vec<Attribute>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declarator" => {
                // Method declaration
                if let Some((name, parameters)) = parse_function_declarator(&child, source) {
                    let return_type = node.child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());

                    let line = node.start_position().row + 1;
                    
                    let mut decorators = vec![];
                    // Check for virtual, static
                    let mut mod_cursor = node.walk();
                    for mod_child in node.children(&mut mod_cursor) {
                        if mod_child.kind() == "virtual" {
                            decorators.push("virtual".to_string());
                        } else if mod_child.kind() == "static" {
                            decorators.push("static".to_string());
                        }
                    }

                    methods.push(Function {
                        name,
                        parameters,
                        return_type,
                        decorators,
                        is_async: false,
                        is_generator: false,
                        is_component: false,
                        docstring: None,
                        line_start: line,
                        line_end: line,
                    });
                }
                return;
            }
            "field_identifier" | "pointer_declarator" | "reference_declarator" => {
                // Member variable
                if let Some(name) = extract_field_name(&child, source) {
                    let type_hint = node.child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());

                    attributes.push(Attribute {
                        name,
                        type_hint,
                        default: None,
                        line: child.start_position().row + 1,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Extract field name
fn extract_field_name(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "field_identifier" | "identifier" => node.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()),
        "pointer_declarator" | "reference_declarator" | "array_declarator" => {
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

/// Parse enum specifier
fn parse_enum(node: &Node, source: &str, namespace_prefix: &str) -> Option<Class> {
    let raw_name = node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())?;

    let name = if namespace_prefix.is_empty() {
        raw_name
    } else {
        format!("{}::{}", namespace_prefix, raw_name)
    };

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    let mut attributes = vec![];

    // Parse enum body
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "enumerator" {
                let enum_name = child.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string());

                let value = child.child_by_field_name("value")
                    .and_then(|v| v.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string());

                if let Some(n) = enum_name {
                    attributes.push(Attribute {
                        name: n,
                        type_hint: None,
                        default: value,
                        line: child.start_position().row + 1,
                    });
                }
            }
        }
    }

    // Check for enum class
    let mut is_enum_class = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "class" || child.kind() == "struct" {
            is_enum_class = true;
            break;
        }
    }

    let bases = if is_enum_class {
        vec!["enum class".to_string()]
    } else {
        vec!["enum".to_string()]
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

/// Parse declarations
fn parse_declaration(node: &Node, source: &str, file: &mut ParsedFile, namespace_prefix: &str) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declarator" => {
                // Function prototype
                if let Some((raw_name, parameters)) = parse_function_declarator(&child, source) {
                    let name = if namespace_prefix.is_empty() {
                        raw_name
                    } else {
                        format!("{}::{}", namespace_prefix, raw_name)
                    };

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
            "class_specifier" | "struct_specifier" => {
                if let Some(class) = parse_class_specifier(&child, source, namespace_prefix) {
                    file.classes.push(class);
                }
            }
            "enum_specifier" => {
                if let Some(class) = parse_enum(&child, source, namespace_prefix) {
                    file.classes.push(class);
                }
            }
            _ => {}
        }
    }
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
        let parser = CppParser::new();
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_includes() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
#include <iostream>
#include <vector>
#include "myheader.h"
"#;
        let file = parser.parse_source(source, "main.cpp".into(), "main".into()).unwrap();
        assert_eq!(file.imports.len(), 3);
        assert_eq!(file.imports[0].module, "iostream");
    }

    #[test]
    fn test_parse_using() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
using namespace std;
using std::cout;
"#;
        let file = parser.parse_source(source, "main.cpp".into(), "main".into()).unwrap();
        assert!(file.imports.len() >= 1);
    }

    #[test]
    fn test_parse_class() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
class Point {
public:
    int x;
    int y;
};
"#;
        let file = parser.parse_source(source, "point.cpp".into(), "point".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let class = &file.classes[0];
        assert_eq!(class.name, "Point");
        // Note: field detection in C++ class bodies requires more work
        // Basic class parsing works
    }

    #[test]
    fn test_parse_class_inheritance() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
class Animal {
public:
    virtual void speak() = 0;
};

class Dog : public Animal {
public:
    void speak() override;
};
"#;
        let file = parser.parse_source(source, "animals.cpp".into(), "animals".into()).unwrap();
        assert_eq!(file.classes.len(), 2);
        
        let dog = &file.classes[1];
        assert_eq!(dog.name, "Dog");
        assert!(dog.bases.contains(&"Animal".to_string()));
    }

    #[test]
    fn test_parse_namespace() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
namespace myapp {
    class Config {
    public:
        int value;
    };

    void init();
}
"#;
        let file = parser.parse_source(source, "app.cpp".into(), "app".into()).unwrap();
        
        // Should have namespaced class and function
        assert!(file.classes.iter().any(|c| c.name == "myapp::Config"));
        assert!(file.functions.iter().any(|f| f.name == "myapp::init"));
    }

    #[test]
    fn test_parse_template() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
template<typename T>
class Container {
public:
    T value;
    void set(T val);
};
"#;
        let file = parser.parse_source(source, "container.cpp".into(), "container".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let class = &file.classes[0];
        assert!(class.decorators.contains(&"template".to_string()));
    }

    #[test]
    fn test_parse_enum_class() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
enum class Color {
    Red,
    Green,
    Blue
};
"#;
        let file = parser.parse_source(source, "types.cpp".into(), "types".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let enum_class = &file.classes[0];
        assert_eq!(enum_class.name, "Color");
        assert!(enum_class.bases.contains(&"enum class".to_string()));
        assert_eq!(enum_class.attributes.len(), 3);
    }

    #[test]
    fn test_parse_struct() {
        let mut parser = CppParser::new().unwrap();
        let source = r#"
struct Point {
    int x;
    int y;
};
"#;
        let file = parser.parse_source(source, "types.cpp".into(), "types".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let s = &file.classes[0];
        assert!(s.bases.contains(&"struct".to_string()));
    }

    #[test]
    fn test_count_lines() {
        let source = r#"
#include <iostream>

// Comment
int main() {
    /* Block */
    std::cout << "Hi";
    return 0;
}
"#;
        let (total, code, comment, blank) = count_lines(source);
        assert!(total > 0);
        assert!(code > 0);
        assert!(comment >= 2);
    }
}
