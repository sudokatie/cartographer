//! Cartographer - Generate architecture docs from codebases
//!
//! Analyzes Python codebases and generates architecture documentation
//! as a static HTML site.

pub mod analysis;
pub mod cli;
pub mod config;
pub mod error;
pub mod output;
pub mod parser;

// Re-export main types
pub use analysis::{CodeGraph, FileId, ClassId, FunctionId, FileNode, ClassNode, FunctionNode, Edge, EdgeKind};
pub use cli::{Args, Command};
pub use config::Config;
pub use error::{Error, Result};
pub use parser::{ParsedFile, PythonParser, Parameter};
