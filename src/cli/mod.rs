//! CLI module for Cartographer

mod args;

pub use args::{Args, Command};

use crate::config::Config;
use crate::error::Result;
use std::path::Path;
use std::process::ExitCode;

/// Run the CLI application
pub fn run() -> ExitCode {
    let args = Args::parse_args();
    
    match execute(args) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn execute(args: Args) -> Result<()> {
    match args.command {
        Command::Analyze {
            path,
            output,
            exclude,
            include,
            config,
            format,
            depth,
            no_diagrams,
            verbose,
        } => {
            // Load config file if it exists
            let mut cfg = if let Some(config_path) = &config {
                Config::load_or_default(config_path)
            } else {
                let default_path = Path::new("cartographer.toml");
                Config::load_or_default(default_path)
            };

            // Merge CLI arguments (CLI takes precedence)
            cfg.merge_cli(
                Some(output.clone()),
                exclude,
                Some(format.clone()),
                Some(depth),
                no_diagrams,
            );

            // Handle include patterns
            if !include.is_empty() {
                cfg.analysis.include = include;
            }

            if verbose {
                println!("Analyzing: {}", path.display());
                println!("Output: {}", cfg.output.directory.display());
                println!("Format: {:?}", cfg.output.format);
                println!("Depth: {}", cfg.analysis.max_depth);
                println!("Diagrams: {}", cfg.diagrams.enabled);
                println!("Include: {:?}", cfg.analysis.include);
                println!("Exclude: {:?}", cfg.analysis.exclude);
            }

            if !path.exists() {
                return Err(crate::error::Error::PathNotFound(path));
            }

            println!("Analysis not yet implemented");
            Ok(())
        }

        Command::Serve { path, port } => {
            if !path.exists() {
                return Err(crate::error::Error::PathNotFound(path));
            }

            println!("Serving {} on http://localhost:{}", path.display(), port);
            println!("Server not yet implemented");
            Ok(())
        }

        Command::Version => {
            println!("cartographer {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
