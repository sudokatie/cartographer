//! CLI module for Cartographer

mod args;

pub use args::{Args, Command};

use crate::analysis::Analyzer;
use crate::config::Config;
use crate::error::Result;
use crate::output::{HtmlConfig, HtmlGenerator};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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
            no_explain,
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
                no_explain,
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
                println!("LLM Explain: {}", cfg.llm.enabled);
                println!("Include: {:?}", cfg.analysis.include);
                println!("Exclude: {:?}", cfg.analysis.exclude);
            }

            if !path.exists() {
                return Err(crate::error::Error::PathNotFound(path));
            }

            // Create and run analyzer
            let mut analyzer = Analyzer::new(cfg.clone())?.with_verbose(verbose);

            println!("Discovering files...");
            let counts = analyzer.file_counts(&path)?;
            
            // Build language breakdown string
            let mut langs = Vec::new();
            if counts.python > 0 {
                langs.push(format!("{} Python", counts.python));
            }
            if counts.javascript > 0 {
                langs.push(format!("{} JavaScript", counts.javascript));
            }
            if counts.typescript > 0 {
                langs.push(format!("{} TypeScript", counts.typescript));
            }
            
            if langs.is_empty() {
                println!("Found {} source files", counts.total());
            } else if langs.len() == 1 {
                println!("Found {} files", langs[0]);
            } else {
                println!("Found {} source files ({})", counts.total(), langs.join(", "));
            }

            println!("Analyzing codebase...");
            let analysis = analyzer.analyze(&path)?;

            println!(
                "Analysis complete: {} files, {} classes, {} functions",
                analysis.graph.stats().files,
                analysis.graph.stats().classes,
                analysis.graph.stats().functions
            );

            if !analysis.parse_errors.is_empty() {
                println!("\nParse errors ({}):", analysis.parse_errors.len());
                for (path, err) in analysis.parse_errors.iter().take(5) {
                    println!("  {}: {}", path.display(), err);
                }
                if analysis.parse_errors.len() > 5 {
                    println!("  ... and {} more", analysis.parse_errors.len() - 5);
                }
            }

            // Get project name from config or directory
            let project_name = if cfg.project.name == "Untitled Project" || cfg.project.name.is_empty() {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Project")
                    .to_string()
            } else {
                cfg.project.name.clone()
            };

            // Generate output based on format
            match format.as_str() {
                "html" => {
                    println!("\nGenerating HTML documentation...");
                    let html_config = HtmlConfig {
                        output_dir: cfg.output.directory.clone(),
                        project_name,
                        generate_diagrams: cfg.diagrams.enabled,
                        copy_assets: true,
                    };

                    let generator = HtmlGenerator::new(html_config)?;
                    let report = generator.generate(&analysis)?;

                    println!("{}", report.summary());
                    println!("Documentation written to: {}", cfg.output.directory.display());
                }
                "json" => {
                    println!("\nGenerating JSON output...");
                    let json = serde_json::to_string_pretty(&analysis.graph)?;
                    let output_path = cfg.output.directory.join("analysis.json");
                    std::fs::create_dir_all(&cfg.output.directory)?;
                    std::fs::write(&output_path, json)?;
                    println!("JSON written to: {}", output_path.display());
                }
                "markdown" => {
                    println!("\nGenerating Markdown output...");
                    let md = generate_markdown(&analysis, &project_name);
                    let output_path = cfg.output.directory.join("README.md");
                    std::fs::create_dir_all(&cfg.output.directory)?;
                    std::fs::write(&output_path, md)?;
                    println!("Markdown written to: {}", output_path.display());
                }
                _ => {
                    return Err(crate::error::Error::Other(format!(
                        "Unknown format: {}",
                        format
                    )));
                }
            }

            Ok(())
        }

        Command::Serve { path, port } => {
            if !path.exists() {
                return Err(crate::error::Error::PathNotFound(path));
            }

            println!("Serving {} on http://localhost:{}", path.display(), port);
            println!("Press Ctrl+C to stop");

            serve_directory(&path, port)?;

            Ok(())
        }

        Command::Version => {
            println!("cartographer {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

/// Simple HTTP server for serving static files
fn serve_directory(root: &Path, port: u16) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .map_err(|e| crate::error::Error::Other(format!("Failed to bind to port {}: {}", port, e)))?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let root = root.to_path_buf();
                std::thread::spawn(move || {
                    if let Err(e) = handle_request(stream, &root) {
                        eprintln!("Request error: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Connection error: {}", e),
        }
    }

    Ok(())
}

/// Handle a single HTTP request
fn handle_request(mut stream: TcpStream, root: &Path) -> Result<()> {
    let mut buffer = [0; 4096];
    let n = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..n]);

    // Parse request line
    let request_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    if parts.len() < 2 {
        send_response(&mut stream, 400, "Bad Request", "text/plain", b"Bad Request")?;
        return Ok(());
    }

    let method = parts[0];
    let path = parts[1];

    if method != "GET" {
        send_response(&mut stream, 405, "Method Not Allowed", "text/plain", b"Method Not Allowed")?;
        return Ok(());
    }

    // Decode URL path and resolve to file
    let url_path = urlparse(path);
    let file_path = if url_path == "/" {
        root.join("index.html")
    } else {
        let relative = url_path.trim_start_matches('/');
        root.join(relative)
    };

    // Security: prevent path traversal
    let canonical = match file_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            send_response(&mut stream, 404, "Not Found", "text/plain", b"Not Found")?;
            return Ok(());
        }
    };

    let root_canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if !canonical.starts_with(&root_canonical) {
        send_response(&mut stream, 403, "Forbidden", "text/plain", b"Forbidden")?;
        return Ok(());
    }

    // Handle directory by looking for index.html
    let final_path = if canonical.is_dir() {
        canonical.join("index.html")
    } else {
        canonical
    };

    // Read and serve file
    match std::fs::read(&final_path) {
        Ok(content) => {
            let content_type = guess_content_type(&final_path);
            send_response(&mut stream, 200, "OK", content_type, &content)?;
            println!("200 {} {}", method, path);
        }
        Err(_) => {
            send_response(&mut stream, 404, "Not Found", "text/plain", b"Not Found")?;
            println!("404 {} {}", method, path);
        }
    }

    Ok(())
}

/// Send an HTTP response
fn send_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status_code,
        status_text,
        content_type,
        body.len()
    );

    stream.write_all(response.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;

    Ok(())
}

/// Guess content type from file extension
fn guess_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        Some("md") => "text/markdown; charset=utf-8",
        Some("mmd") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Simple URL decoding
fn urlparse(s: &str) -> String {
    // Split off query string
    let path = s.split('?').next().unwrap_or(s);
    
    // Decode percent-encoded characters
    let mut result = String::new();
    let mut chars = path.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else {
            result.push(c);
        }
    }
    
    result
}

/// Generate markdown documentation
fn generate_markdown(analysis: &crate::analysis::AnalysisResult, project_name: &str) -> String {
    let stats = analysis.graph.stats();
    let mut md = String::new();

    md.push_str(&format!("# {}\n\n", project_name));
    md.push_str("## Project Statistics\n\n");
    md.push_str(&format!("- **Files:** {}\n", stats.files));
    md.push_str(&format!("- **Classes:** {}\n", stats.classes));
    md.push_str(&format!("- **Functions:** {}\n", stats.functions));
    md.push_str(&format!("- **Lines of Code:** {}\n", stats.code_lines));
    md.push('\n');

    md.push_str("## Modules\n\n");
    for module in &analysis.modules {
        md.push_str(&format!("### {}\n\n", module.name));
        md.push_str(&format!("- **Files:** {}\n", module.files.len()));
        md.push_str(&format!("- **Type:** {:?}\n", module.module_type));
        md.push('\n');
    }

    md.push_str("## Classes\n\n");
    for (_, class) in analysis.graph.all_classes() {
        md.push_str(&format!("### {}\n\n", class.name));
        if !class.bases.is_empty() {
            md.push_str(&format!("Inherits from: {}\n\n", class.bases.join(", ")));
        }
        if let Some(doc) = &class.docstring {
            md.push_str(&format!("{}\n\n", doc));
        }
    }

    md
}
