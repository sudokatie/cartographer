use std::path::PathBuf;
use thiserror::Error;

/// Cartographer error types
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse config file: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Config validation error: {0}")]
    ConfigValidation(String),

    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),

    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),

    #[error("Parse error in {path}: {message}")]
    Parse { path: PathBuf, message: String },

    #[error("Template error: {0}")]
    Template(#[from] tera::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Glob pattern error: {0}")]
    GlobPattern(#[from] glob::PatternError),

    #[error("Directory walk error: {0}")]
    WalkDir(#[from] walkdir::Error),

    #[error("Analysis error: {0}")]
    Analysis(String),

    #[error("Parser error: {0}")]
    Parser(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("{0}")]
    Other(String),
}

/// Result type alias for Cartographer operations
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a config validation error
    pub fn config_validation(msg: impl Into<String>) -> Self {
        Error::ConfigValidation(msg.into())
    }

    /// Create a parse error
    pub fn parse(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Error::Parse {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create an analysis error
    pub fn analysis(msg: impl Into<String>) -> Self {
        Error::Analysis(msg.into())
    }

    /// Create a parser error
    pub fn parser(msg: impl Into<String>) -> Self {
        Error::Parser(msg.into())
    }

    /// Create a generic error
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }

    /// Create an LLM error
    pub fn llm(msg: impl Into<String>) -> Self {
        Error::Llm(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_error_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(err.to_string().contains("IO error"));
    }

    #[test]
    fn test_path_not_found_display() {
        let err = Error::PathNotFound(PathBuf::from("/some/path"));
        assert_eq!(err.to_string(), "Path not found: /some/path");
    }

    #[test]
    fn test_parse_error_display() {
        let err = Error::parse("/foo/bar.py", "unexpected token");
        assert!(err.to_string().contains("/foo/bar.py"));
        assert!(err.to_string().contains("unexpected token"));
    }

    #[test]
    fn test_config_validation_display() {
        let err = Error::config_validation("depth must be positive");
        assert_eq!(err.to_string(), "Config validation error: depth must be positive");
    }

    #[test]
    fn test_analysis_error() {
        let err = Error::analysis("circular dependency detected");
        assert_eq!(err.to_string(), "Analysis error: circular dependency detected");
    }

    #[test]
    fn test_other_error() {
        let err = Error::other("something went wrong");
        assert_eq!(err.to_string(), "something went wrong");
    }

    #[test]
    fn test_parser_error() {
        let err = Error::parser("unexpected token");
        assert_eq!(err.to_string(), "Parser error: unexpected token");
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Error>();
    }
}
