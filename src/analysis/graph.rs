// Code graph for representing project structure

use crate::parser::{Class, Constant, Function, Import, Parameter, ParsedFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unique identifier for a file in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileId(pub usize);

/// Unique identifier for a class in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClassId(pub usize);

/// Unique identifier for a function in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionId(pub usize);

/// Unique identifier for any node in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeId {
    File(FileId),
    Class(ClassId),
    Function(FunctionId),
}

impl From<FileId> for NodeId {
    fn from(id: FileId) -> Self {
        NodeId::File(id)
    }
}

impl From<ClassId> for NodeId {
    fn from(id: ClassId) -> Self {
        NodeId::Class(id)
    }
}

impl From<FunctionId> for NodeId {
    fn from(id: FunctionId) -> Self {
        NodeId::Function(id)
    }
}

/// A file node in the code graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    /// File path relative to project root
    pub path: PathBuf,
    /// Module name
    pub module_name: String,
    /// File docstring
    pub docstring: Option<String>,
    /// Total lines
    pub total_lines: usize,
    /// Lines of code
    pub code_lines: usize,
    /// Comment lines
    pub comment_lines: usize,
    /// IDs of classes in this file
    pub classes: Vec<ClassId>,
    /// IDs of functions in this file
    pub functions: Vec<FunctionId>,
    /// Import statements
    pub imports: Vec<Import>,
    /// Module-level constants
    pub constants: Vec<Constant>,
}

impl FileNode {
    /// Create from a parsed file
    pub fn from_parsed(file: &ParsedFile, classes: Vec<ClassId>, functions: Vec<FunctionId>) -> Self {
        Self {
            path: file.path.clone(),
            module_name: file.module_name.clone(),
            docstring: file.docstring.clone(),
            total_lines: file.total_lines,
            code_lines: file.code_lines,
            comment_lines: file.comment_lines,
            classes,
            functions,
            imports: file.imports.clone(),
            constants: file.constants.clone(),
        }
    }

    /// Get all imported modules
    pub fn imported_modules(&self) -> impl Iterator<Item = &str> {
        self.imports.iter().map(|i| i.module.as_str())
    }
}

/// A class node in the code graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassNode {
    /// Class name
    pub name: String,
    /// File containing this class
    pub file: FileId,
    /// Class docstring
    pub docstring: Option<String>,
    /// Base classes (as written)
    pub bases: Vec<String>,
    /// Decorators
    pub decorators: Vec<String>,
    /// Method IDs
    pub methods: Vec<FunctionId>,
    /// Line range
    pub line_start: usize,
    pub line_end: usize,
}

impl ClassNode {
    /// Create from a parsed class
    pub fn from_parsed(class: &Class, file: FileId, methods: Vec<FunctionId>) -> Self {
        Self {
            name: class.name.clone(),
            file,
            docstring: class.docstring.clone(),
            bases: class.bases.clone(),
            decorators: class.decorators.clone(),
            methods,
            line_start: class.line_start,
            line_end: class.line_end,
        }
    }

    /// Check if this class inherits from a given base
    pub fn inherits_from(&self, base: &str) -> bool {
        self.bases.iter().any(|b| b == base || b.ends_with(&format!(".{}", base)))
    }
}

/// A function node in the code graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionNode {
    /// Function name
    pub name: String,
    /// File containing this function
    pub file: FileId,
    /// Class containing this function (if method)
    pub class: Option<ClassId>,
    /// Function docstring
    pub docstring: Option<String>,
    /// Parameters with types and defaults (per SPECS.md 4.1)
    pub parameters: Vec<Parameter>,
    /// Function signature (convenience string representation)
    pub signature: String,
    /// Return type
    pub return_type: Option<String>,
    /// Is async
    pub is_async: bool,
    /// Decorators
    pub decorators: Vec<String>,
    /// Line range
    pub line_start: usize,
    pub line_end: usize,
}

impl FunctionNode {
    /// Create from a parsed function
    pub fn from_parsed(func: &Function, file: FileId, class: Option<ClassId>) -> Self {
        Self {
            name: func.name.clone(),
            file,
            class,
            docstring: func.docstring.clone(),
            parameters: func.parameters.clone(),
            signature: func.signature(),
            return_type: func.return_type.clone(),
            is_async: func.is_async,
            decorators: func.decorators.clone(),
            line_start: func.line_start,
            line_end: func.line_end,
        }
    }

    /// Check if this is a method (has a class)
    pub fn is_method(&self) -> bool {
        self.class.is_some()
    }

    /// Check if this appears to be a test function
    pub fn is_test(&self) -> bool {
        self.name.starts_with("test_") || self.decorators.iter().any(|d| d.contains("test"))
    }
}

/// Kind of edge in the code graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    /// File A imports from File B
    Imports,
    /// Class A inherits from Class B
    Inherits,
    /// Function A calls Function B (detected statically)
    Calls,
    /// Class/Function is defined in File
    DefinedIn,
}

/// An edge in the code graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Source node
    pub from: NodeId,
    /// Target node
    pub to: NodeId,
    /// Kind of relationship
    pub kind: EdgeKind,
}

impl Edge {
    pub fn new(from: impl Into<NodeId>, to: impl Into<NodeId>, kind: EdgeKind) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            kind,
        }
    }

    pub fn imports(from: FileId, to: FileId) -> Self {
        Self::new(from, to, EdgeKind::Imports)
    }

    pub fn inherits(from: ClassId, to: ClassId) -> Self {
        Self::new(from, to, EdgeKind::Inherits)
    }
}

/// The code graph containing all nodes and edges
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CodeGraph {
    /// File nodes by ID
    pub files: HashMap<FileId, FileNode>,
    /// Class nodes by ID
    pub classes: HashMap<ClassId, ClassNode>,
    /// Function nodes by ID
    pub functions: HashMap<FunctionId, FunctionNode>,
    /// All edges
    pub edges: Vec<Edge>,
    /// Map from file path to file ID
    path_to_file: HashMap<PathBuf, FileId>,
    /// Map from module name to file ID
    module_to_file: HashMap<String, FileId>,
    /// Next IDs
    next_file_id: usize,
    next_class_id: usize,
    next_function_id: usize,
}

impl CodeGraph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file from a parsed file
    pub fn add_file(&mut self, parsed: &ParsedFile) -> FileId {
        let file_id = FileId(self.next_file_id);
        self.next_file_id += 1;

        // Add classes and collect their IDs
        let mut class_ids = Vec::new();
        for class in &parsed.classes {
            let class_id = self.add_class_internal(class, file_id);
            class_ids.push(class_id);
        }

        // Add top-level functions
        let mut func_ids = Vec::new();
        for func in &parsed.functions {
            let func_id = self.add_function_internal(func, file_id, None);
            func_ids.push(func_id);
        }

        let node = FileNode::from_parsed(parsed, class_ids, func_ids);
        
        self.path_to_file.insert(parsed.path.clone(), file_id);
        self.module_to_file.insert(parsed.module_name.clone(), file_id);
        self.files.insert(file_id, node);

        file_id
    }

    fn add_class_internal(&mut self, class: &Class, file_id: FileId) -> ClassId {
        let class_id = ClassId(self.next_class_id);
        self.next_class_id += 1;

        // Add methods
        let mut method_ids = Vec::new();
        for method in &class.methods {
            let func_id = self.add_function_internal(method, file_id, Some(class_id));
            method_ids.push(func_id);
        }

        let node = ClassNode::from_parsed(class, file_id, method_ids);
        self.classes.insert(class_id, node);
        
        class_id
    }

    fn add_function_internal(&mut self, func: &Function, file_id: FileId, class_id: Option<ClassId>) -> FunctionId {
        let func_id = FunctionId(self.next_function_id);
        self.next_function_id += 1;

        let node = FunctionNode::from_parsed(func, file_id, class_id);
        self.functions.insert(func_id, node);
        
        func_id
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Get a file by ID
    pub fn get_file(&self, id: FileId) -> Option<&FileNode> {
        self.files.get(&id)
    }

    /// Get a class by ID
    pub fn get_class(&self, id: ClassId) -> Option<&ClassNode> {
        self.classes.get(&id)
    }

    /// Get a function by ID
    pub fn get_function(&self, id: FunctionId) -> Option<&FunctionNode> {
        self.functions.get(&id)
    }

    /// Find file by path
    pub fn file_by_path(&self, path: &PathBuf) -> Option<FileId> {
        self.path_to_file.get(path).copied()
    }

    /// Find file by module name
    pub fn file_by_module(&self, module: &str) -> Option<FileId> {
        self.module_to_file.get(module).copied()
    }

    /// Get all files that a file imports
    pub fn imports_of(&self, file: FileId) -> Vec<FileId> {
        self.edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Imports && e.from == NodeId::File(file))
            .filter_map(|e| match e.to {
                NodeId::File(id) => Some(id),
                _ => None,
            })
            .collect()
    }

    /// Get all files that import a given file
    pub fn imported_by(&self, file: FileId) -> Vec<FileId> {
        self.edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Imports && e.to == NodeId::File(file))
            .filter_map(|e| match e.from {
                NodeId::File(id) => Some(id),
                _ => None,
            })
            .collect()
    }

    /// Iterate over all files
    pub fn all_files(&self) -> impl Iterator<Item = (FileId, &FileNode)> {
        self.files.iter().map(|(&id, node)| (id, node))
    }

    /// Iterate over all classes
    pub fn all_classes(&self) -> impl Iterator<Item = (ClassId, &ClassNode)> {
        self.classes.iter().map(|(&id, node)| (id, node))
    }

    /// Iterate over all functions
    pub fn all_functions(&self) -> impl Iterator<Item = (FunctionId, &FunctionNode)> {
        self.functions.iter().map(|(&id, node)| (id, node))
    }

    /// Get statistics about the graph
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            files: self.files.len(),
            classes: self.classes.len(),
            functions: self.functions.len(),
            edges: self.edges.len(),
            total_lines: self.files.values().map(|f| f.total_lines).sum(),
            code_lines: self.files.values().map(|f| f.code_lines).sum(),
        }
    }
}

/// Statistics about the code graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub files: usize,
    pub classes: usize,
    pub functions: usize,
    pub edges: usize,
    pub total_lines: usize,
    pub code_lines: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ParsedFile;
    use std::path::PathBuf;

    fn make_parsed_file(name: &str) -> ParsedFile {
        ParsedFile::new(PathBuf::from(format!("{}.py", name)), name.to_string())
    }

    #[test]
    fn test_empty_graph() {
        let graph = CodeGraph::new();
        assert_eq!(graph.files.len(), 0);
        assert_eq!(graph.classes.len(), 0);
        assert_eq!(graph.functions.len(), 0);
    }

    #[test]
    fn test_add_file() {
        let mut graph = CodeGraph::new();
        let parsed = make_parsed_file("test");
        let id = graph.add_file(&parsed);
        
        assert_eq!(id.0, 0);
        assert!(graph.get_file(id).is_some());
        assert_eq!(graph.get_file(id).unwrap().module_name, "test");
    }

    #[test]
    fn test_file_by_path() {
        let mut graph = CodeGraph::new();
        let parsed = make_parsed_file("mymodule");
        let id = graph.add_file(&parsed);
        
        assert_eq!(graph.file_by_path(&PathBuf::from("mymodule.py")), Some(id));
        assert_eq!(graph.file_by_module("mymodule"), Some(id));
    }

    #[test]
    fn test_add_edge() {
        let mut graph = CodeGraph::new();
        let file1 = graph.add_file(&make_parsed_file("a"));
        let file2 = graph.add_file(&make_parsed_file("b"));
        
        graph.add_edge(Edge::imports(file1, file2));
        
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.imports_of(file1), vec![file2]);
        assert_eq!(graph.imported_by(file2), vec![file1]);
    }

    #[test]
    fn test_imports_of_empty() {
        let mut graph = CodeGraph::new();
        let file1 = graph.add_file(&make_parsed_file("lonely"));
        
        assert!(graph.imports_of(file1).is_empty());
        assert!(graph.imported_by(file1).is_empty());
    }

    #[test]
    fn test_graph_stats() {
        let mut graph = CodeGraph::new();
        let mut file = make_parsed_file("test");
        file.total_lines = 100;
        file.code_lines = 80;
        graph.add_file(&file);
        
        let stats = graph.stats();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.total_lines, 100);
        assert_eq!(stats.code_lines, 80);
    }

    #[test]
    fn test_node_id_conversion() {
        let file_id = FileId(1);
        let class_id = ClassId(2);
        let func_id = FunctionId(3);
        
        assert_eq!(NodeId::from(file_id), NodeId::File(file_id));
        assert_eq!(NodeId::from(class_id), NodeId::Class(class_id));
        assert_eq!(NodeId::from(func_id), NodeId::Function(func_id));
    }

    #[test]
    fn test_class_inherits_from() {
        let class = ClassNode {
            name: "Child".to_string(),
            file: FileId(0),
            docstring: None,
            bases: vec!["Parent".to_string(), "module.Mixin".to_string()],
            decorators: Vec::new(),
            methods: Vec::new(),
            line_start: 1,
            line_end: 10,
        };
        
        assert!(class.inherits_from("Parent"));
        assert!(class.inherits_from("Mixin"));
        assert!(!class.inherits_from("Unknown"));
    }

    #[test]
    fn test_function_is_test() {
        let test_func = FunctionNode {
            name: "test_something".to_string(),
            file: FileId(0),
            class: None,
            docstring: None,
            parameters: Vec::new(),
            signature: "def test_something()".to_string(),
            return_type: None,
            is_async: false,
            decorators: Vec::new(),
            line_start: 1,
            line_end: 5,
        };
        
        let regular_func = FunctionNode {
            name: "do_something".to_string(),
            file: FileId(0),
            class: None,
            docstring: None,
            parameters: Vec::new(),
            signature: "def do_something()".to_string(),
            return_type: None,
            is_async: false,
            decorators: Vec::new(),
            line_start: 1,
            line_end: 5,
        };
        
        assert!(test_func.is_test());
        assert!(!regular_func.is_test());
    }
}
