// Go parser using tree-sitter

use crate::error::{Error, Result};
use crate::parser::ast::{
    Class, Constant, Function, Import, ImportKind, Parameter, ParameterKind, ParsedFile,
};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Parser for Go source files
pub struct GoParser {
    parser: Parser,
}

impl GoParser {
    /// Create a new Go parser
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_go::language();
        parser.set_language(&language).map_err(|e| {
            Error::Parser(format!("Failed to set Go language: {}", e))
        })?;

        Ok(Self { parser })
    }

    /// Parse a Go file
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedFile> {
        let source = std::fs::read_to_string(path).map_err(|e| {
            Error::Io(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        let module_name = path_to_module_name(path);
        self.parse_source(&source, path.to_path_buf(), module_name)
    }

    /// Parse Go source code
    pub fn parse_source(
        &mut self,
        source: &str,
        path: std::path::PathBuf,
        module_name: String,
    ) -> Result<ParsedFile> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            Error::parser("Failed to parse Go source")
        })?;

        let root = tree.root_node();
        let mut file = ParsedFile::new(path, module_name);

        // Walk the tree and extract constructs
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "package_clause" => {
                    // Extract package name as docstring for context
                    // In tree-sitter-go, the package name is a child node, not a field
                    let mut pkg_cursor = child.walk();
                    for pkg_child in child.children(&mut pkg_cursor) {
                        if pkg_child.kind() == "package_identifier" {
                            if let Ok(pkg) = pkg_child.utf8_text(source.as_bytes()) {
                                file.docstring = Some(format!("package {}", pkg));
                                break;
                            }
                        }
                    }
                }
                "import_declaration" => {
                    let imports = parse_imports(&child, source);
                    file.imports.extend(imports);
                }
                "type_declaration" => {
                    // Contains type_spec nodes for struct, interface, type alias
                    let mut spec_cursor = child.walk();
                    for spec in child.children(&mut spec_cursor) {
                        if spec.kind() == "type_spec" {
                            if let Some(class) = parse_type_spec(&spec, source) {
                                file.classes.push(class);
                            }
                        }
                    }
                }
                "function_declaration" => {
                    if let Some(func) = parse_function(&child, source) {
                        file.functions.push(func);
                    }
                }
                "method_declaration" => {
                    if let Some(func) = parse_method(&child, source) {
                        file.functions.push(func);
                    }
                }
                "const_declaration" | "var_declaration" => {
                    let constants = parse_var_or_const(&child, source);
                    file.constants.extend(constants);
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

impl Default for GoParser {
    fn default() -> Self {
        Self::new().expect("Failed to create GoParser")
    }
}

/// Convert file path to Go package/module name
fn path_to_module_name(path: &Path) -> String {
    let stem = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // In Go, the package is typically the directory name, but we use filename
    stem.to_string()
}

/// Parse import declaration (can have single or multiple imports)
fn parse_imports(node: &Node, source: &str) -> Vec<Import> {
    let mut imports = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "import_spec" {
            if let Some(import) = parse_import_spec(&child, source) {
                imports.push(import);
            }
        } else if child.kind() == "import_spec_list" {
            // Multiple imports in parentheses
            let mut list_cursor = child.walk();
            for spec in child.children(&mut list_cursor) {
                if spec.kind() == "import_spec" {
                    if let Some(import) = parse_import_spec(&spec, source) {
                        imports.push(import);
                    }
                }
            }
        }
    }

    imports
}

/// Parse a single import spec
fn parse_import_spec(node: &Node, source: &str) -> Option<Import> {
    let mut cursor = node.walk();
    let mut path_str = None;

    for child in node.children(&mut cursor) {
        if child.kind() == "interpreted_string_literal" {
            let text = child.utf8_text(source.as_bytes()).ok()?;
            // Remove quotes
            path_str = Some(text.trim_matches('"').to_string());
        }
    }

    let module = path_str?;
    
    Some(Import {
        module,
        names: vec![],
        kind: ImportKind::Direct,
        line: node.start_position().row + 1,
    })
}

/// Parse type_spec (struct, interface, type alias)
fn parse_type_spec(node: &Node, source: &str) -> Option<Class> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let type_node = node.child_by_field_name("type")?;
    let type_kind = type_node.kind();

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Determine if struct or interface
    let bases = match type_kind {
        "struct_type" => vec![],
        "interface_type" => vec!["Interface".to_string()],
        _ => vec!["TypeAlias".to_string()],
    };

    // Extract methods for interfaces
    let mut methods = vec![];
    if type_kind == "interface_type" {
        // Interface body contains method_elem nodes which contain method_spec
        let mut cursor = type_node.walk();
        for child in type_node.children(&mut cursor) {
            // Could be method_spec, method_elem, or nested
            if child.kind() == "method_spec" || child.kind() == "method_elem" {
                if let Some(method) = parse_method_spec(&child, source) {
                    methods.push(method);
                }
            } else {
                // Look for nested method specs
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "method_spec" || inner.kind() == "method_elem" {
                        if let Some(method) = parse_method_spec(&inner, source) {
                            methods.push(method);
                        }
                    }
                }
            }
        }
    }

    let docstring = extract_comment(node, source);

    Some(Class {
        name,
        bases,
        docstring,
        methods,
        decorators: vec![],
        attributes: vec![],
        line_start,
        line_end,
    })
}

/// Parse interface method specification
fn parse_method_spec(node: &Node, source: &str) -> Option<Function> {
    // Try field name first, fall back to finding identifier child
    let name_node = if let Some(n) = node.child_by_field_name("name") {
        Some(n)
    } else {
        // Search for field_identifier
        let mut cursor = node.walk();
        let mut found = None;
        for child in node.children(&mut cursor) {
            if child.kind() == "field_identifier" {
                found = Some(child);
                break;
            }
        }
        found
    };

    let name = name_node?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract parameters - look for parameter_list
    let mut parameters = vec![];
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "parameter_list" {
            parameters = parse_parameters(&child, source);
            break;
        }
    }

    // Extract return type
    let return_type = node.child_by_field_name("result")
        .and_then(|r| r.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    Some(Function {
        name,
        parameters,
        return_type,
        docstring: None,
        decorators: vec![],
        is_async: false,
        is_generator: false,
        is_component: false,
        line_start,
        line_end,
    })
}

/// Parse function declaration
fn parse_function(node: &Node, source: &str) -> Option<Function> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract parameters
    let mut parameters = vec![];
    if let Some(params) = node.child_by_field_name("parameters") {
        parameters = parse_parameters(&params, source);
    }

    // Extract return type
    let return_type = node.child_by_field_name("result")
        .and_then(|r| r.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    let docstring = extract_comment(node, source);

    Some(Function {
        name,
        parameters,
        return_type,
        docstring,
        decorators: vec![],
        is_async: false,
        is_generator: false,
        is_component: false,
        line_start,
        line_end,
    })
}

/// Parse method declaration (function with receiver)
fn parse_method(node: &Node, source: &str) -> Option<Function> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    // Get receiver type for method name prefixing
    let receiver_type = node.child_by_field_name("receiver")
        .and_then(|r| {
            // The receiver contains parameter declarations
            let mut cursor = r.walk();
            for child in r.children(&mut cursor) {
                if child.kind() == "parameter_declaration" {
                    if let Some(type_node) = child.child_by_field_name("type") {
                        let type_text = type_node.utf8_text(source.as_bytes()).ok()?;
                        // Strip pointer prefix if present
                        return Some(type_text.trim_start_matches('*').to_string());
                    }
                }
            }
            None
        });

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract parameters
    let mut parameters = vec![];
    if let Some(params) = node.child_by_field_name("parameters") {
        parameters = parse_parameters(&params, source);
    }

    // Extract return type
    let return_type = node.child_by_field_name("result")
        .and_then(|r| r.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    let docstring = extract_comment(node, source);

    // Prefix method name with receiver type
    let full_name = if let Some(recv) = receiver_type {
        format!("{}.{}", recv, name)
    } else {
        name
    };

    Some(Function {
        name: full_name,
        parameters,
        return_type,
        docstring,
        decorators: vec![],
        is_async: false,
        is_generator: false,
        is_component: false,
        line_start,
        line_end,
    })
}

/// Parse parameter list
fn parse_parameters(node: &Node, source: &str) -> Vec<Parameter> {
    let mut parameters = vec![];
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "parameter_declaration" {
            // Can have multiple names for same type: a, b int
            let type_hint = child.child_by_field_name("type")
                .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                .map(|s| s.to_string());

            // Get all identifiers (parameter names)
            let mut name_cursor = child.walk();
            let mut found_names = vec![];
            for name_child in child.children(&mut name_cursor) {
                if name_child.kind() == "identifier" {
                    if let Ok(name) = name_child.utf8_text(source.as_bytes()) {
                        found_names.push(name.to_string());
                    }
                }
            }

            // If no names found, it might be just a type (like in interface methods)
            if found_names.is_empty() {
                if let Some(ref t) = type_hint {
                    parameters.push(Parameter {
                        name: "_".to_string(),
                        type_hint: Some(t.clone()),
                        default: None,
                        kind: ParameterKind::Regular,
                    });
                }
            } else {
                for name in found_names {
                    parameters.push(Parameter {
                        name,
                        type_hint: type_hint.clone(),
                        default: None,
                        kind: ParameterKind::Regular,
                    });
                }
            }
        } else if child.kind() == "variadic_parameter_declaration" {
            // ...args style
            let type_hint = child.child_by_field_name("type")
                .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                .map(|s| format!("...{}", s));
            
            let name = child.child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("args")
                .to_string();

            parameters.push(Parameter {
                name,
                type_hint,
                default: None,
                kind: ParameterKind::Args,
            });
        }
    }

    parameters
}

/// Parse const or var declarations
fn parse_var_or_const(node: &Node, source: &str) -> Vec<Constant> {
    let mut constants = vec![];
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "const_spec" || child.kind() == "var_spec" {
            // Can have multiple names
            let mut names = vec![];
            let mut type_hint = None;
            let mut value = None;

            let mut spec_cursor = child.walk();
            for spec_child in child.children(&mut spec_cursor) {
                match spec_child.kind() {
                    "identifier" => {
                        if let Ok(name) = spec_child.utf8_text(source.as_bytes()) {
                            names.push(name.to_string());
                        }
                    }
                    "type_identifier" | "qualified_type" | "pointer_type" | "slice_type" | 
                    "array_type" | "map_type" | "channel_type" | "function_type" => {
                        type_hint = spec_child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
                    }
                    "expression_list" => {
                        value = spec_child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
                    }
                    _ => {}
                }
            }

            let line = child.start_position().row + 1;
            for name in names {
                constants.push(Constant {
                    name,
                    type_hint: type_hint.clone(),
                    value: value.clone(),
                    line,
                });
            }
        }
    }

    constants
}

/// Extract preceding comment as docstring
fn extract_comment(node: &Node, source: &str) -> Option<String> {
    if let Some(prev) = node.prev_sibling() {
        if prev.kind() == "comment" {
            let text = prev.utf8_text(source.as_bytes()).ok()?;
            let doc = text.trim_start_matches("//")
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
        let parser = GoParser::new();
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_package() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

import "fmt"

func main() {
    fmt.Println("hello")
}
"#;
        let file = parser.parse_source(source, "main.go".into(), "main".into()).unwrap();
        assert_eq!(file.docstring, Some("package main".to_string()));
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.imports[0].module, "fmt");
    }

    #[test]
    fn test_parse_imports() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

import (
    "fmt"
    "os"
    "path/filepath"
)
"#;
        let file = parser.parse_source(source, "main.go".into(), "main".into()).unwrap();
        assert_eq!(file.imports.len(), 3);
        assert_eq!(file.imports[0].module, "fmt");
        assert_eq!(file.imports[1].module, "os");
        assert_eq!(file.imports[2].module, "path/filepath");
    }

    #[test]
    fn test_parse_struct() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

type Point struct {
    X float64
    Y float64
}
"#;
        let file = parser.parse_source(source, "main.go".into(), "main".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        assert_eq!(file.classes[0].name, "Point");
        assert!(file.classes[0].bases.is_empty());
    }

    #[test]
    fn test_parse_interface() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

type Reader interface {
    Read(p []byte) (n int, err error)
}
"#;
        let file = parser.parse_source(source, "main.go".into(), "main".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        assert_eq!(file.classes[0].name, "Reader");
        assert!(file.classes[0].bases.contains(&"Interface".to_string()));
        assert_eq!(file.classes[0].methods.len(), 1);
        assert_eq!(file.classes[0].methods[0].name, "Read");
    }

    #[test]
    fn test_parse_function() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

func Add(a, b int) int {
    return a + b
}
"#;
        let file = parser.parse_source(source, "main.go".into(), "main".into()).unwrap();
        assert_eq!(file.functions.len(), 1);
        assert_eq!(file.functions[0].name, "Add");
        assert_eq!(file.functions[0].parameters.len(), 2);
        assert_eq!(file.functions[0].return_type, Some("int".to_string()));
    }

    #[test]
    fn test_parse_method() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

type Point struct {
    X, Y float64
}

func (p *Point) Distance(other Point) float64 {
    return 0.0
}
"#;
        let file = parser.parse_source(source, "main.go".into(), "main".into()).unwrap();
        // Should have struct Point and method Point.Distance
        assert_eq!(file.classes.len(), 1);
        assert!(!file.functions.is_empty());
        // Method should be prefixed with receiver type
        assert!(file.functions.iter().any(|f| f.name == "Point.Distance"));
    }

    #[test]
    fn test_parse_const() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

const (
    MaxSize = 1024
    MinSize = 1
)

var globalVar = "hello"
"#;
        let file = parser.parse_source(source, "main.go".into(), "main".into()).unwrap();
        assert_eq!(file.constants.len(), 3);
        assert!(file.constants.iter().any(|c| c.name == "MaxSize"));
        assert!(file.constants.iter().any(|c| c.name == "MinSize"));
        assert!(file.constants.iter().any(|c| c.name == "globalVar"));
    }

    #[test]
    fn test_parse_variadic() {
        let mut parser = GoParser::new().unwrap();
        let source = r#"
package main

func Printf(format string, args ...interface{}) {
}
"#;
        let file = parser.parse_source(source, "main.go".into(), "main".into()).unwrap();
        assert_eq!(file.functions.len(), 1);
        assert_eq!(file.functions[0].parameters.len(), 2);
        assert_eq!(file.functions[0].parameters[1].kind, ParameterKind::Args);
    }

    #[test]
    fn test_count_lines() {
        let source = r#"
// A comment
package main

/* block
   comment */

func main() {}
"#;
        let (total, code, comment, blank) = count_lines(source);
        assert!(total > 0);
        assert!(code > 0);
        assert!(comment >= 2);
        assert!(blank >= 1);
    }
}
