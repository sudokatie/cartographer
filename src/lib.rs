//! Cartographer - Generate architecture docs from codebases
//!
//! Analyzes Python codebases and generates architecture documentation
//! as a static HTML site.

pub mod cli;
pub mod config;
pub mod error;

// Re-export main types
pub use cli::{Args, Command};
pub use config::Config;
pub use error::{Error, Result};
