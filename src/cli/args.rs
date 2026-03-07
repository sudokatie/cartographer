//! CLI argument parsing

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Generate architecture docs from codebases
#[derive(Parser, Debug)]
#[command(name = "cartographer")]
#[command(about = "Generate architecture docs from codebases")]
#[command(version)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

impl Args {
    pub fn parse_args() -> Self {
        Parser::parse()
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Analyze a codebase and generate documentation
    Analyze {
        /// Path to the codebase to analyze
        path: PathBuf,

        /// Output directory
        #[arg(short, long, default_value = "./cartographer-docs")]
        output: PathBuf,

        /// Glob patterns to exclude (can be repeated)
        #[arg(long)]
        exclude: Vec<String>,

        /// Glob patterns to include (can be repeated)
        #[arg(long, default_value = "**/*.py")]
        include: Vec<String>,

        /// Config file path
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Output format (html, json, markdown)
        #[arg(long, default_value = "html")]
        format: String,

        /// Max dependency depth to display
        #[arg(long, default_value = "5")]
        depth: usize,

        /// Skip diagram generation
        #[arg(long)]
        no_diagrams: bool,

        /// Skip LLM-generated explanations
        #[arg(long)]
        no_explain: bool,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Use incremental analysis (cache unchanged files)
        #[arg(long)]
        incremental: bool,
    },

    /// Serve generated documentation locally
    Serve {
        /// Path to the generated docs
        path: PathBuf,

        /// Port to serve on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },

    /// Export dependency graph to various formats
    Export {
        /// Path to the codebase to analyze
        path: PathBuf,

        /// Output format: dot, mermaid, svg, png
        #[arg(short, long, default_value = "mermaid")]
        format: String,

        /// Output file path (default: stdout for text formats)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Max dependency depth
        #[arg(long, default_value = "5")]
        depth: usize,

        /// Filter to specific module(s) (glob pattern, repeatable)
        #[arg(long)]
        module: Vec<String>,

        /// Include patterns for source files
        #[arg(long, default_value = "**/*.py")]
        include: Vec<String>,

        /// Exclude patterns for source files
        #[arg(long)]
        exclude: Vec<String>,

        /// Graph direction: TB, LR, BT, RL
        #[arg(long, default_value = "TB")]
        direction: String,

        /// Exclude external/third-party dependencies
        #[arg(long)]
        no_externals: bool,

        /// Group nodes by module (subgraphs)
        #[arg(long)]
        cluster: bool,
    },

    /// Show version information
    Version,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_defaults() {
        let args = Args::try_parse_from(["cartographer", "analyze", "./src"]).unwrap();
        match args.command {
            Command::Analyze { path, output, depth, format, include, incremental, .. } => {
                assert_eq!(path, PathBuf::from("./src"));
                assert_eq!(output, PathBuf::from("./cartographer-docs"));
                assert_eq!(depth, 5);
                assert_eq!(format, "html");
                assert_eq!(include, vec!["**/*.py".to_string()]);
                assert!(!incremental);
            }
            _ => panic!("Expected Analyze command"),
        }
    }
    
    #[test]
    fn test_analyze_incremental() {
        let args = Args::try_parse_from(["cartographer", "analyze", "./src", "--incremental"]).unwrap();
        match args.command {
            Command::Analyze { incremental, .. } => {
                assert!(incremental);
            }
            _ => panic!("Expected Analyze command"),
        }
    }

    #[test]
    fn test_analyze_with_options() {
        let args = Args::try_parse_from([
            "cartographer", "analyze", "./project",
            "--output", "/tmp/docs",
            "--exclude", "tests/**",
            "--include", "**/*.py",
            "--include", "**/*.pyi",
            "--config", "custom.toml",
            "--format", "json",
            "--depth", "10",
            "--no-diagrams",
            "--no-explain",
            "--verbose",
            "--incremental",
        ]).unwrap();
        
        match args.command {
            Command::Analyze { 
                path, output, exclude, include, config, 
                format, depth, no_diagrams, no_explain, verbose, incremental 
            } => {
                assert_eq!(path, PathBuf::from("./project"));
                assert_eq!(output, PathBuf::from("/tmp/docs"));
                assert_eq!(exclude, vec!["tests/**".to_string()]);
                assert_eq!(include, vec!["**/*.py".to_string(), "**/*.pyi".to_string()]);
                assert_eq!(config, Some(PathBuf::from("custom.toml")));
                assert_eq!(format, "json");
                assert_eq!(depth, 10);
                assert!(no_diagrams);
                assert!(no_explain);
                assert!(verbose);
                assert!(incremental);
            }
            _ => panic!("Expected Analyze command"),
        }
    }

    #[test]
    fn test_serve_defaults() {
        let args = Args::try_parse_from(["cartographer", "serve", "./docs"]).unwrap();
        match args.command {
            Command::Serve { path, port } => {
                assert_eq!(path, PathBuf::from("./docs"));
                assert_eq!(port, 8080);
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn test_serve_with_port() {
        let args = Args::try_parse_from(["cartographer", "serve", "./docs", "--port", "3000"]).unwrap();
        match args.command {
            Command::Serve { port, .. } => {
                assert_eq!(port, 3000);
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn test_version_command() {
        let args = Args::try_parse_from(["cartographer", "version"]).unwrap();
        assert!(matches!(args.command, Command::Version));
    }
}
