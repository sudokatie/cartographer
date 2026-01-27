use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "cartographer")]
#[command(about = "Generate architecture docs from codebases")]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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

        /// Output format
        #[arg(long, default_value = "html")]
        format: String,

        /// Max dependency depth to display
        #[arg(long, default_value = "5")]
        depth: usize,

        /// Skip diagram generation
        #[arg(long)]
        no_diagrams: bool,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Serve generated documentation locally
    Serve {
        /// Path to the generated docs
        path: PathBuf,

        /// Port to serve on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
}

fn main() -> ExitCode {
    let args = Args::parse();

    match args.command {
        Command::Analyze {
            path,
            output,
            exclude,
            format,
            depth,
            no_diagrams,
            verbose,
        } => {
            if verbose {
                println!("Analyzing: {}", path.display());
                println!("Output: {}", output.display());
                println!("Format: {}", format);
                println!("Depth: {}", depth);
                println!("Diagrams: {}", !no_diagrams);
                if !exclude.is_empty() {
                    println!("Excludes: {:?}", exclude);
                }
            }

            if !path.exists() {
                eprintln!("Error: Path does not exist: {}", path.display());
                return ExitCode::FAILURE;
            }

            println!("Analysis not yet implemented");
            ExitCode::SUCCESS
        }

        Command::Serve { path, port } => {
            if !path.exists() {
                eprintln!("Error: Path does not exist: {}", path.display());
                return ExitCode::FAILURE;
            }

            println!("Serving {} on http://localhost:{}", path.display(), port);
            println!("Server not yet implemented");
            ExitCode::SUCCESS
        }
    }
}
