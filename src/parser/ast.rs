// AST types for parsed Python code
//
// These types represent the abstract syntax tree extracted from Python source files.
// They are designed to be serializable for caching and debugging.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A parsed Python file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedFile {
    /// File path relative to project root
    pub path: PathBuf,
    /// Module name derived from path
    pub module_name: String,
    /// Module-level docstring
    pub docstring: Option<String>,
    /// All imports in the file
    pub imports: Vec<Import>,
    /// All classes defined in the file
    pub classes: Vec<Class>,
    /// Top-level functions (not methods)
    pub functions: Vec<Function>,
    /// Module-level constants
    pub constants: Vec<Constant>,
    /// Total lines in file
    pub total_lines: usize,
    /// Lines of code (excluding blanks and comments)
    pub code_lines: usize,
    /// Comment lines
    pub comment_lines: usize,
}

impl ParsedFile {
    /// Create a new parsed file with basic info
    pub fn new(path: PathBuf, module_name: String) -> Self {
        Self {
            path,
            module_name,
            docstring: None,
            imports: Vec::new(),
            classes: Vec::new(),
            functions: Vec::new(),
            constants: Vec::new(),
            total_lines: 0,
            code_lines: 0,
            comment_lines: 0,
        }
    }

    /// Check if file has any content
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty() && self.functions.is_empty() && self.constants.is_empty()
    }
}

/// An import statement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Import {
    /// The module being imported
    pub module: String,
    /// Specific names imported (for `from x import y`)
    pub names: Vec<ImportedName>,
    /// Import kind
    pub kind: ImportKind,
    /// Line number
    pub line: usize,
}

impl Import {
    /// Create a simple `import x` style import
    pub fn simple(module: &str, line: usize) -> Self {
        Self {
            module: module.to_string(),
            names: Vec::new(),
            kind: ImportKind::Direct,
            line,
        }
    }

    /// Create a `from x import y` style import
    pub fn from_import(module: &str, names: Vec<ImportedName>, line: usize) -> Self {
        Self {
            module: module.to_string(),
            names,
            kind: ImportKind::From,
            line,
        }
    }

    /// Create a relative import
    pub fn relative(module: &str, names: Vec<ImportedName>, level: usize, line: usize) -> Self {
        Self {
            module: module.to_string(),
            names,
            kind: ImportKind::Relative { level },
            line,
        }
    }
}

/// A single imported name with optional alias
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportedName {
    /// Original name
    pub name: String,
    /// Alias (from `as` clause)
    pub alias: Option<String>,
}

impl ImportedName {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            alias: None,
        }
    }

    pub fn with_alias(name: &str, alias: &str) -> Self {
        Self {
            name: name.to_string(),
            alias: Some(alias.to_string()),
        }
    }

    /// Get the name as used in code (alias if present, otherwise original)
    pub fn used_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

/// Kind of import statement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImportKind {
    /// `import x` or `import x as y`
    Direct,
    /// `from x import y`
    From,
    /// `from . import y` or `from ..x import y`
    Relative { level: usize },
}

impl ImportKind {
    pub fn is_relative(&self) -> bool {
        matches!(self, ImportKind::Relative { .. })
    }
}

/// A class definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Class {
    /// Class name
    pub name: String,
    /// Class docstring
    pub docstring: Option<String>,
    /// Base classes (as written, not resolved)
    pub bases: Vec<String>,
    /// Decorators applied to the class
    pub decorators: Vec<String>,
    /// Methods defined in the class
    pub methods: Vec<Function>,
    /// Class attributes (assigned in class body, not __init__)
    pub attributes: Vec<Attribute>,
    /// Starting line number
    pub line_start: usize,
    /// Ending line number
    pub line_end: usize,
}

impl Class {
    pub fn new(name: &str, line_start: usize) -> Self {
        Self {
            name: name.to_string(),
            docstring: None,
            bases: Vec::new(),
            decorators: Vec::new(),
            methods: Vec::new(),
            attributes: Vec::new(),
            line_start,
            line_end: line_start,
        }
    }

    /// Check if this is a dataclass
    pub fn is_dataclass(&self) -> bool {
        self.decorators.iter().any(|d| d.contains("dataclass"))
    }

    /// Check if this appears to be an exception class
    pub fn is_exception(&self) -> bool {
        self.bases.iter().any(|b| b.contains("Exception") || b.contains("Error"))
    }

    /// Get public methods (not starting with _)
    pub fn public_methods(&self) -> impl Iterator<Item = &Function> {
        self.methods.iter().filter(|m| !m.name.starts_with('_'))
    }

    /// Get special methods (__x__)
    pub fn special_methods(&self) -> impl Iterator<Item = &Function> {
        self.methods.iter().filter(|m| m.name.starts_with("__") && m.name.ends_with("__"))
    }
}

/// A class attribute
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attribute {
    /// Attribute name
    pub name: String,
    /// Type annotation if present
    pub type_hint: Option<String>,
    /// Default value as string (if present)
    pub default: Option<String>,
    /// Line number
    pub line: usize,
}

impl Attribute {
    pub fn new(name: &str, line: usize) -> Self {
        Self {
            name: name.to_string(),
            type_hint: None,
            default: None,
            line,
        }
    }
}

/// A function or method definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Function {
    /// Function name
    pub name: String,
    /// Function docstring
    pub docstring: Option<String>,
    /// Parameters with types and defaults
    pub parameters: Vec<Parameter>,
    /// Return type annotation
    pub return_type: Option<String>,
    /// Decorators applied
    pub decorators: Vec<String>,
    /// Whether this is an async function
    pub is_async: bool,
    /// Whether this is a generator (contains yield)
    pub is_generator: bool,
    /// Whether this is a React component (returns JSX)
    pub is_component: bool,
    /// Starting line number
    pub line_start: usize,
    /// Ending line number
    pub line_end: usize,
}

impl Function {
    pub fn new(name: &str, line_start: usize) -> Self {
        Self {
            name: name.to_string(),
            docstring: None,
            parameters: Vec::new(),
            return_type: None,
            decorators: Vec::new(),
            is_async: false,
            is_generator: false,
            is_component: false,
            line_start,
            line_end: line_start,
        }
    }

    /// Check if function name follows React component convention (PascalCase)
    pub fn has_component_name(&self) -> bool {
        self.name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
    }

    /// Check if this is a private function (starts with _)
    pub fn is_private(&self) -> bool {
        self.name.starts_with('_') && !self.name.starts_with("__")
    }

    /// Check if this is a special method (__x__)
    pub fn is_special(&self) -> bool {
        self.name.starts_with("__") && self.name.ends_with("__")
    }

    /// Check if this is a property
    pub fn is_property(&self) -> bool {
        self.decorators.iter().any(|d| d == "property" || d.ends_with(".getter"))
    }

    /// Check if this is a classmethod
    pub fn is_classmethod(&self) -> bool {
        self.decorators.iter().any(|d| d == "classmethod")
    }

    /// Check if this is a staticmethod
    pub fn is_staticmethod(&self) -> bool {
        self.decorators.iter().any(|d| d == "staticmethod")
    }

    /// Get the function signature as a string
    pub fn signature(&self) -> String {
        let params: Vec<String> = self.parameters.iter().map(|p| p.to_string()).collect();
        let ret = self.return_type.as_ref().map(|r| format!(" -> {}", r)).unwrap_or_default();
        let prefix = if self.is_async { "async " } else { "" };
        format!("{}def {}({}){}", prefix, self.name, params.join(", "), ret)
    }
}

/// A function parameter
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Parameter {
    /// Parameter name
    pub name: String,
    /// Type annotation
    pub type_hint: Option<String>,
    /// Default value as string
    pub default: Option<String>,
    /// Parameter kind
    pub kind: ParameterKind,
}

impl Parameter {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            type_hint: None,
            default: None,
            kind: ParameterKind::Regular,
        }
    }

    pub fn with_type(name: &str, type_hint: &str) -> Self {
        Self {
            name: name.to_string(),
            type_hint: Some(type_hint.to_string()),
            default: None,
            kind: ParameterKind::Regular,
        }
    }

    pub fn with_default(name: &str, default: &str) -> Self {
        Self {
            name: name.to_string(),
            type_hint: None,
            default: Some(default.to_string()),
            kind: ParameterKind::Regular,
        }
    }
}

impl std::fmt::Display for Parameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        
        match self.kind {
            ParameterKind::Args => s.push('*'),
            ParameterKind::Kwargs => s.push_str("**"),
            _ => {}
        }
        
        s.push_str(&self.name);
        
        if let Some(ref t) = self.type_hint {
            s.push_str(": ");
            s.push_str(t);
        }
        
        if let Some(ref d) = self.default {
            if self.type_hint.is_some() {
                s.push_str(" = ");
            } else {
                s.push('=');
            }
            s.push_str(d);
        }
        
        write!(f, "{}", s)
    }
}

/// Kind of function parameter
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParameterKind {
    /// Regular positional or keyword parameter
    Regular,
    /// *args
    Args,
    /// **kwargs
    Kwargs,
    /// Positional-only (before /)
    PositionalOnly,
    /// Keyword-only (after *)
    KeywordOnly,
}

/// A module-level constant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Constant {
    /// Constant name (usually ALL_CAPS)
    pub name: String,
    /// Type annotation if present
    pub type_hint: Option<String>,
    /// Value as string
    pub value: Option<String>,
    /// Line number
    pub line: usize,
}

impl Constant {
    pub fn new(name: &str, line: usize) -> Self {
        Self {
            name: name.to_string(),
            type_hint: None,
            value: None,
            line,
        }
    }

    /// Check if name follows ALL_CAPS convention
    pub fn is_conventional(&self) -> bool {
        self.name.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_file_new() {
        let file = ParsedFile::new(PathBuf::from("test.py"), "test".to_string());
        assert_eq!(file.module_name, "test");
        assert!(file.is_empty());
    }

    #[test]
    fn test_import_simple() {
        let imp = Import::simple("os", 1);
        assert_eq!(imp.module, "os");
        assert_eq!(imp.kind, ImportKind::Direct);
        assert!(imp.names.is_empty());
    }

    #[test]
    fn test_import_from() {
        let names = vec![
            ImportedName::new("path"),
            ImportedName::with_alias("join", "pjoin"),
        ];
        let imp = Import::from_import("os", names, 1);
        assert_eq!(imp.kind, ImportKind::From);
        assert_eq!(imp.names.len(), 2);
        assert_eq!(imp.names[1].used_name(), "pjoin");
    }

    #[test]
    fn test_import_relative() {
        let names = vec![ImportedName::new("helper")];
        let imp = Import::relative("utils", names, 2, 1);
        assert!(imp.kind.is_relative());
        if let ImportKind::Relative { level } = imp.kind {
            assert_eq!(level, 2);
        }
    }

    #[test]
    fn test_imported_name_used_name() {
        let name = ImportedName::new("foo");
        assert_eq!(name.used_name(), "foo");
        
        let aliased = ImportedName::with_alias("foo", "bar");
        assert_eq!(aliased.used_name(), "bar");
    }

    #[test]
    fn test_class_new() {
        let class = Class::new("MyClass", 10);
        assert_eq!(class.name, "MyClass");
        assert_eq!(class.line_start, 10);
        assert!(class.methods.is_empty());
    }

    #[test]
    fn test_class_is_dataclass() {
        let mut class = Class::new("Data", 1);
        assert!(!class.is_dataclass());
        
        class.decorators.push("dataclass".to_string());
        assert!(class.is_dataclass());
    }

    #[test]
    fn test_class_is_exception() {
        let mut class = Class::new("MyError", 1);
        assert!(!class.is_exception());
        
        class.bases.push("Exception".to_string());
        assert!(class.is_exception());
    }

    #[test]
    fn test_function_new() {
        let func = Function::new("my_func", 5);
        assert_eq!(func.name, "my_func");
        assert!(!func.is_async);
        assert!(!func.is_private());
    }

    #[test]
    fn test_function_is_private() {
        let private = Function::new("_private", 1);
        assert!(private.is_private());
        
        let special = Function::new("__init__", 1);
        assert!(!special.is_private());
        assert!(special.is_special());
        
        let public = Function::new("public", 1);
        assert!(!public.is_private());
    }

    #[test]
    fn test_function_is_property() {
        let mut func = Function::new("value", 1);
        assert!(!func.is_property());
        
        func.decorators.push("property".to_string());
        assert!(func.is_property());
    }

    #[test]
    fn test_function_signature() {
        let mut func = Function::new("greet", 1);
        func.parameters.push(Parameter::with_type("name", "str"));
        func.return_type = Some("str".to_string());
        
        assert_eq!(func.signature(), "def greet(name: str) -> str");
    }

    #[test]
    fn test_function_async_signature() {
        let mut func = Function::new("fetch", 1);
        func.is_async = true;
        func.parameters.push(Parameter::new("url"));
        
        assert_eq!(func.signature(), "async def fetch(url)");
    }

    #[test]
    fn test_parameter_display() {
        let simple = Parameter::new("x");
        assert_eq!(simple.to_string(), "x");
        
        let typed = Parameter::with_type("x", "int");
        assert_eq!(typed.to_string(), "x: int");
        
        let with_default = Parameter::with_default("x", "10");
        assert_eq!(with_default.to_string(), "x=10");
        
        let mut full = Parameter::new("x");
        full.type_hint = Some("int".to_string());
        full.default = Some("10".to_string());
        assert_eq!(full.to_string(), "x: int = 10");
    }

    #[test]
    fn test_parameter_args_kwargs() {
        let mut args = Parameter::new("args");
        args.kind = ParameterKind::Args;
        assert_eq!(args.to_string(), "*args");
        
        let mut kwargs = Parameter::new("kwargs");
        kwargs.kind = ParameterKind::Kwargs;
        assert_eq!(kwargs.to_string(), "**kwargs");
    }

    #[test]
    fn test_constant_new() {
        let const_ = Constant::new("MAX_SIZE", 1);
        assert!(const_.is_conventional());
        
        let not_const = Constant::new("maxSize", 1);
        assert!(!not_const.is_conventional());
    }

    #[test]
    fn test_attribute_new() {
        let attr = Attribute::new("count", 5);
        assert_eq!(attr.name, "count");
        assert_eq!(attr.line, 5);
        assert!(attr.type_hint.is_none());
    }

    #[test]
    fn test_serialization() {
        let file = ParsedFile::new(PathBuf::from("test.py"), "test".to_string());
        let json = serde_json::to_string(&file).expect("serialize");
        let parsed: ParsedFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.module_name, "test");
    }
}
