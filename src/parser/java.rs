// Java parser using tree-sitter

use crate::error::{Error, Result};
use crate::parser::ast::{
    Attribute, Class, Function, Import, ImportKind, Parameter, ParameterKind, ParsedFile,
};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Parser for Java source files
pub struct JavaParser {
    parser: Parser,
}

impl JavaParser {
    /// Create a new Java parser
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_java::language();
        parser.set_language(&language).map_err(|e| {
            Error::Parser(format!("Failed to set Java language: {}", e))
        })?;

        Ok(Self { parser })
    }

    /// Parse a Java file
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedFile> {
        let source = std::fs::read_to_string(path).map_err(|e| {
            Error::Io(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        let module_name = path_to_module_name(path);
        self.parse_source(&source, path.to_path_buf(), module_name)
    }

    /// Parse Java source code
    pub fn parse_source(
        &mut self,
        source: &str,
        path: std::path::PathBuf,
        module_name: String,
    ) -> Result<ParsedFile> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            Error::parser("Failed to parse Java source")
        })?;

        let root = tree.root_node();
        let mut file = ParsedFile::new(path, module_name);

        // Walk the tree and extract constructs
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "package_declaration" => {
                    if let Some(pkg) = parse_package(&child, source) {
                        file.docstring = Some(format!("package {}", pkg));
                    }
                }
                "import_declaration" => {
                    if let Some(import) = parse_import(&child, source) {
                        file.imports.push(import);
                    }
                }
                "class_declaration" => {
                    if let Some(class) = parse_class(&child, source) {
                        file.classes.push(class);
                    }
                }
                "interface_declaration" => {
                    if let Some(class) = parse_interface(&child, source) {
                        file.classes.push(class);
                    }
                }
                "enum_declaration" => {
                    if let Some(class) = parse_enum(&child, source) {
                        file.classes.push(class);
                    }
                }
                "record_declaration" => {
                    if let Some(class) = parse_record(&child, source) {
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

impl Default for JavaParser {
    fn default() -> Self {
        Self::new().expect("Failed to create JavaParser")
    }
}

/// Convert file path to Java class name
fn path_to_module_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Parse package declaration
fn parse_package(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
            return child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
        }
    }
    None
}

/// Parse import declaration
fn parse_import(node: &Node, source: &str) -> Option<Import> {
    let mut cursor = node.walk();
    let mut is_static = false;
    let mut is_wildcard = false;
    let mut module_path = None;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "static" => is_static = true,
            "scoped_identifier" | "identifier" => {
                module_path = child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
            }
            "asterisk" => is_wildcard = true,
            _ => {}
        }
    }

    let mut module = module_path?;
    if is_wildcard {
        module.push_str(".*");
    }
    if is_static {
        module = format!("static {}", module);
    }

    Some(Import {
        module,
        names: vec![],
        kind: ImportKind::Direct,
        line: node.start_position().row + 1,
    })
}

/// Parse class declaration
fn parse_class(node: &Node, source: &str) -> Option<Class> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract superclass and interfaces
    let mut bases = vec![];
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "superclass" => {
                // Try field access first
                if let Some(type_node) = child.child_by_field_name("type") {
                    if let Ok(text) = type_node.utf8_text(source.as_bytes()) {
                        bases.push(text.to_string());
                    }
                } else {
                    // Fall back to iterating children for type_identifier or generic_type
                    let mut sc_cursor = child.walk();
                    for sc_child in child.children(&mut sc_cursor) {
                        if sc_child.kind() == "type_identifier" || sc_child.kind() == "generic_type" {
                            if let Ok(text) = sc_child.utf8_text(source.as_bytes()) {
                                bases.push(text.to_string());
                            }
                            break;
                        }
                    }
                }
            }
            "super_interfaces" => {
                // Look for type_list or direct type_identifier children
                let mut iface_cursor = child.walk();
                for iface_child in child.children(&mut iface_cursor) {
                    if iface_child.kind() == "type_list" {
                        let mut type_cursor = iface_child.walk();
                        for type_child in iface_child.children(&mut type_cursor) {
                            if type_child.kind() == "type_identifier" || type_child.kind() == "generic_type" {
                                if let Ok(text) = type_child.utf8_text(source.as_bytes()) {
                                    bases.push(text.to_string());
                                }
                            }
                        }
                    } else if iface_child.kind() == "type_identifier" || iface_child.kind() == "generic_type" {
                        if let Ok(text) = iface_child.utf8_text(source.as_bytes()) {
                            bases.push(text.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Extract methods and fields from class body
    let mut methods = vec![];
    let mut attributes = vec![];

    if let Some(body) = node.child_by_field_name("body") {
        let mut body_cursor = body.walk();
        for body_child in body.children(&mut body_cursor) {
            match body_child.kind() {
                "method_declaration" | "constructor_declaration" => {
                    if let Some(method) = parse_method(&body_child, source) {
                        methods.push(method);
                    }
                }
                "field_declaration" => {
                    let fields = parse_field(&body_child, source);
                    attributes.extend(fields);
                }
                _ => {}
            }
        }
    }

    let docstring = extract_javadoc(node, source);

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

/// Parse interface declaration
fn parse_interface(node: &Node, source: &str) -> Option<Class> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract extended interfaces
    let mut bases = vec!["Interface".to_string()];
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "extends_interfaces" {
            let mut ext_cursor = child.walk();
            for ext_child in child.children(&mut ext_cursor) {
                if ext_child.kind() == "type_list" {
                    let mut type_cursor = ext_child.walk();
                    for type_child in ext_child.children(&mut type_cursor) {
                        if type_child.kind() == "type_identifier" || type_child.kind() == "generic_type" {
                            if let Ok(text) = type_child.utf8_text(source.as_bytes()) {
                                bases.push(text.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Extract method signatures from interface body
    let mut methods = vec![];
    if let Some(body) = node.child_by_field_name("body") {
        let mut body_cursor = body.walk();
        for body_child in body.children(&mut body_cursor) {
            if body_child.kind() == "method_declaration" {
                if let Some(method) = parse_method(&body_child, source) {
                    methods.push(method);
                }
            }
        }
    }

    let docstring = extract_javadoc(node, source);

    Some(Class {
        name,
        bases,
        methods,
        decorators: vec![],
        docstring,
        line_start,
        line_end,
        attributes: vec![],
    })
}

/// Parse enum declaration
fn parse_enum(node: &Node, source: &str) -> Option<Class> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract enum constants as attributes
    let mut attributes = vec![];
    if let Some(body) = node.child_by_field_name("body") {
        let mut body_cursor = body.walk();
        for body_child in body.children(&mut body_cursor) {
            if body_child.kind() == "enum_constant" {
                if let Some(const_name) = body_child.child_by_field_name("name") {
                    if let Ok(text) = const_name.utf8_text(source.as_bytes()) {
                        attributes.push(Attribute {
                            name: text.to_string(),
                            type_hint: Some(name.clone()),
                            default: None,
                            line: body_child.start_position().row + 1,
                        });
                    }
                }
            }
        }
    }

    let docstring = extract_javadoc(node, source);

    Some(Class {
        name,
        bases: vec!["Enum".to_string()],
        methods: vec![],
        decorators: vec![],
        docstring,
        line_start,
        line_end,
        attributes,
    })
}

/// Parse record declaration (Java 16+)
fn parse_record(node: &Node, source: &str) -> Option<Class> {
    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract record components as attributes
    let mut attributes = vec![];
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "formal_parameters" {
            let params = parse_parameters(&child, source);
            for param in params {
                attributes.push(Attribute {
                    name: param.name,
                    type_hint: param.type_hint,
                    default: None,
                    line: line_start,
                });
            }
        }
    }

    let docstring = extract_javadoc(node, source);

    Some(Class {
        name,
        bases: vec!["Record".to_string()],
        methods: vec![],
        decorators: vec![],
        docstring,
        line_start,
        line_end,
        attributes,
    })
}

/// Parse method or constructor declaration
fn parse_method(node: &Node, source: &str) -> Option<Function> {
    let is_constructor = node.kind() == "constructor_declaration";

    let name = node.child_by_field_name("name")?
        .utf8_text(source.as_bytes()).ok()?
        .to_string();

    let line_start = node.start_position().row + 1;
    let line_end = node.end_position().row + 1;

    // Extract return type (not for constructors)
    let return_type = if is_constructor {
        None
    } else {
        node.child_by_field_name("type")
            .and_then(|t| t.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string())
    };

    // Extract parameters
    let parameters = node.child_by_field_name("parameters")
        .map(|p| parse_parameters(&p, source))
        .unwrap_or_default();

    // Extract modifiers as decorators
    let mut decorators = vec![];
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if let Ok(text) = mod_child.utf8_text(source.as_bytes()) {
                    decorators.push(text.to_string());
                }
            }
        }
    }

    let docstring = extract_javadoc(node, source);

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

/// Parse method parameters
fn parse_parameters(node: &Node, source: &str) -> Vec<Parameter> {
    let mut parameters = vec![];
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "formal_parameter" | "spread_parameter" => {
                let is_vararg = child.kind() == "spread_parameter";

                let type_hint = child.child_by_field_name("type")
                    .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                    .map(|s| if is_vararg { format!("{}...", s) } else { s.to_string() });

                let name = child.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("param")
                    .to_string();

                parameters.push(Parameter {
                    name,
                    type_hint,
                    default: None,
                    kind: if is_vararg { ParameterKind::Args } else { ParameterKind::Regular },
                });
            }
            _ => {}
        }
    }

    parameters
}

/// Parse field declaration
fn parse_field(node: &Node, source: &str) -> Vec<Attribute> {
    let mut attributes = vec![];

    let type_hint = node.child_by_field_name("type")
        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string());

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name = child.child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .map(|s| s.to_string());

            let value = child.child_by_field_name("value")
                .and_then(|v| v.utf8_text(source.as_bytes()).ok())
                .map(|s| s.to_string());

            if let Some(name) = name {
                attributes.push(Attribute {
                    name,
                    type_hint: type_hint.clone(),
                    default: value,
                    line: child.start_position().row + 1,
                });
            }
        }
    }

    attributes
}

/// Extract Javadoc comment preceding a node
fn extract_javadoc(node: &Node, source: &str) -> Option<String> {
    // Look for preceding block comment
    let mut sibling = node.prev_sibling();
    while let Some(prev) = sibling {
        match prev.kind() {
            "block_comment" => {
                let text = prev.utf8_text(source.as_bytes()).ok()?;
                if text.starts_with("/**") {
                    // Parse Javadoc
                    let doc = text
                        .trim_start_matches("/**")
                        .trim_end_matches("*/")
                        .lines()
                        .map(|line| line.trim().trim_start_matches('*').trim())
                        .filter(|line| !line.is_empty() && !line.starts_with('@'))
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !doc.is_empty() {
                        return Some(doc);
                    }
                }
                return None;
            }
            "line_comment" => {
                sibling = prev.prev_sibling();
            }
            _ => return None,
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
        let parser = JavaParser::new();
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_package() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
package com.example.app;

public class Main {
}
"#;
        let file = parser.parse_source(source, "Main.java".into(), "Main".into()).unwrap();
        assert_eq!(file.docstring, Some("package com.example.app".to_string()));
    }

    #[test]
    fn test_parse_imports() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

import java.util.List;
import java.util.Map;
import static java.lang.Math.PI;
import java.io.*;

public class Main {}
"#;
        let file = parser.parse_source(source, "Main.java".into(), "Main".into()).unwrap();
        assert_eq!(file.imports.len(), 4);
        assert_eq!(file.imports[0].module, "java.util.List");
        assert_eq!(file.imports[1].module, "java.util.Map");
        assert!(file.imports[2].module.contains("static"));
        assert!(file.imports[3].module.contains("*"));
    }

    #[test]
    fn test_parse_class() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
package com.example;

public class Person {
    private String name;
    private int age;

    public Person(String name, int age) {
        this.name = name;
        this.age = age;
    }

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }
}
"#;
        let file = parser.parse_source(source, "Person.java".into(), "Person".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let class = &file.classes[0];
        assert_eq!(class.name, "Person");
        assert_eq!(class.methods.len(), 3); // constructor + 2 methods
        assert_eq!(class.attributes.len(), 2); // 2 fields
    }

    #[test]
    fn test_parse_class_with_inheritance() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
public class Employee extends Person implements Serializable, Comparable<Employee> {
    private String department;
}
"#;
        let file = parser.parse_source(source, "Employee.java".into(), "Employee".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let class = &file.classes[0];
        assert_eq!(class.name, "Employee");
        assert!(class.bases.contains(&"Person".to_string()));
        assert!(class.bases.contains(&"Serializable".to_string()));
    }

    #[test]
    fn test_parse_interface() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
public interface Repository<T> extends Closeable {
    T findById(long id);
    List<T> findAll();
    void save(T entity);
}
"#;
        let file = parser.parse_source(source, "Repository.java".into(), "Repository".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let interface = &file.classes[0];
        assert_eq!(interface.name, "Repository");
        assert!(interface.bases.contains(&"Interface".to_string()));
        assert_eq!(interface.methods.len(), 3);
    }

    #[test]
    fn test_parse_enum() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
public enum Status {
    PENDING,
    ACTIVE,
    COMPLETED,
    CANCELLED
}
"#;
        let file = parser.parse_source(source, "Status.java".into(), "Status".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let enum_class = &file.classes[0];
        assert_eq!(enum_class.name, "Status");
        assert!(enum_class.bases.contains(&"Enum".to_string()));
        assert_eq!(enum_class.attributes.len(), 4);
    }

    #[test]
    fn test_parse_record() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
public record Point(int x, int y) {}
"#;
        let file = parser.parse_source(source, "Point.java".into(), "Point".into()).unwrap();
        assert_eq!(file.classes.len(), 1);
        
        let record = &file.classes[0];
        assert_eq!(record.name, "Point");
        assert!(record.bases.contains(&"Record".to_string()));
        assert_eq!(record.attributes.len(), 2);
    }

    #[test]
    fn test_parse_method_parameters() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
public class Utils {
    public static String format(String template, Object... args) {
        return String.format(template, args);
    }
}
"#;
        let file = parser.parse_source(source, "Utils.java".into(), "Utils".into()).unwrap();
        let class = &file.classes[0];
        let method = &class.methods[0];
        
        assert_eq!(method.name, "format");
        assert_eq!(method.parameters.len(), 2);
        assert_eq!(method.parameters[0].name, "template");
        assert_eq!(method.parameters[1].kind, ParameterKind::Args);
    }

    #[test]
    fn test_parse_javadoc() {
        let mut parser = JavaParser::new().unwrap();
        let source = r#"
/**
 * A simple calculator class.
 * @author Katie
 */
public class Calculator {
    /**
     * Adds two numbers.
     * @param a First number
     * @param b Second number
     * @return Sum of a and b
     */
    public int add(int a, int b) {
        return a + b;
    }
}
"#;
        let file = parser.parse_source(source, "Calculator.java".into(), "Calculator".into()).unwrap();
        let class = &file.classes[0];
        
        assert!(class.docstring.is_some());
        assert!(class.docstring.as_ref().unwrap().contains("simple calculator"));
        
        let method = &class.methods[0];
        assert!(method.docstring.is_some());
        assert!(method.docstring.as_ref().unwrap().contains("Adds two numbers"));
    }

    #[test]
    fn test_count_lines() {
        let source = r#"
package com.example;

// Single line comment
public class Main {
    /* Block comment */
    public static void main(String[] args) {
        System.out.println("Hello");
    }
}
"#;
        let (total, code, comment, blank) = count_lines(source);
        assert!(total > 0);
        assert!(code > 0);
        assert!(comment >= 2);
        assert!(blank >= 1);
    }
}
