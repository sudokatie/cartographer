// Parser module for extracting AST from source files

pub mod ast;
mod c;
mod cpp;
mod go;
mod java;
mod javascript;
mod python;
mod rust;

pub use ast::*;
pub use c::CParser;
pub use cpp::CppParser;
pub use go::GoParser;
pub use java::JavaParser;
pub use javascript::{JavaScriptParser, JsVariant};
pub use python::PythonParser;
pub use rust::RustParser;
