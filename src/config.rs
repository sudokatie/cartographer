use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub project: ProjectConfig,
    pub analysis: AnalysisConfig,
    pub output: OutputConfig,
    pub diagrams: DiagramConfig,
}

/// Project metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
}

/// Analysis settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisConfig {
    pub exclude: Vec<String>,
    pub include: Vec<String>,
    pub entry_points: Vec<String>,
    pub max_depth: usize,
}

/// Output settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub format: OutputFormat,
    pub directory: PathBuf,
    pub theme: String,
}

/// Diagram settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagramConfig {
    pub enabled: bool,
    pub max_nodes: usize,
    pub layout: DiagramLayout,
}

/// Output format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Html,
    Json,
    Markdown,
}

/// Diagram layout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DiagramLayout {
    #[default]
    Hierarchical,
    Force,
    Radial,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "Untitled Project".to_string(),
            description: None,
            version: None,
        }
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            exclude: vec![
                "tests/**".to_string(),
                "test/**".to_string(),
                "venv/**".to_string(),
                ".venv/**".to_string(),
                "__pycache__/**".to_string(),
                "*.egg-info/**".to_string(),
                ".git/**".to_string(),
            ],
            include: vec!["**/*.py".to_string()],
            entry_points: vec![],
            max_depth: 5,
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: OutputFormat::default(),
            directory: PathBuf::from("./cartographer-docs"),
            theme: "default".to_string(),
        }
    }
}

impl Default for DiagramConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_nodes: 100,
            layout: DiagramLayout::default(),
        }
    }
}

impl Config {
    /// Load config from a TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        config.validate()?;
        Ok(config)
    }

    /// Load config from file or return defaults
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }

    /// Merge CLI arguments into config (CLI takes precedence)
    pub fn merge_cli(
        &mut self,
        output: Option<PathBuf>,
        exclude: Vec<String>,
        format: Option<String>,
        depth: Option<usize>,
        no_diagrams: bool,
    ) {
        if let Some(out) = output {
            self.output.directory = out;
        }

        if !exclude.is_empty() {
            self.analysis.exclude.extend(exclude);
        }

        if let Some(fmt) = format {
            self.output.format = match fmt.as_str() {
                "json" => OutputFormat::Json,
                "markdown" | "md" => OutputFormat::Markdown,
                _ => OutputFormat::Html,
            };
        }

        if let Some(d) = depth {
            self.analysis.max_depth = d;
        }

        if no_diagrams {
            self.diagrams.enabled = false;
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.analysis.max_depth == 0 {
            return Err(Error::config_validation("max_depth must be at least 1"));
        }

        if self.analysis.max_depth > 100 {
            return Err(Error::config_validation("max_depth cannot exceed 100"));
        }

        if self.diagrams.max_nodes == 0 {
            return Err(Error::config_validation("diagram max_nodes must be at least 1"));
        }

        if self.analysis.include.is_empty() {
            return Err(Error::config_validation("at least one include pattern required"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.project.name, "Untitled Project");
        assert_eq!(config.analysis.max_depth, 5);
        assert!(config.diagrams.enabled);
        assert_eq!(config.output.format, OutputFormat::Html);
    }

    #[test]
    fn test_load_valid_config() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[project]
name = "My Project"
description = "Test project"

[analysis]
max_depth = 10

[output]
format = "json"

[diagrams]
enabled = false
"#
        )
        .unwrap();

        let config = Config::load(file.path()).unwrap();
        assert_eq!(config.project.name, "My Project");
        assert_eq!(config.analysis.max_depth, 10);
        assert_eq!(config.output.format, OutputFormat::Json);
        assert!(!config.diagrams.enabled);
    }

    #[test]
    fn test_load_missing_file() {
        let result = Config::load(Path::new("/nonexistent/config.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_max_depth_zero() {
        let mut config = Config::default();
        config.analysis.max_depth = 0;
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_max_depth_too_high() {
        let mut config = Config::default();
        config.analysis.max_depth = 101;
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_empty_include() {
        let mut config = Config::default();
        config.analysis.include.clear();
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_cli_output() {
        let mut config = Config::default();
        config.merge_cli(Some(PathBuf::from("/custom/output")), vec![], None, None, false);
        assert_eq!(config.output.directory, PathBuf::from("/custom/output"));
    }

    #[test]
    fn test_merge_cli_exclude() {
        let mut config = Config::default();
        let initial_excludes = config.analysis.exclude.len();
        config.merge_cli(None, vec!["node_modules/**".to_string()], None, None, false);
        assert_eq!(config.analysis.exclude.len(), initial_excludes + 1);
    }

    #[test]
    fn test_merge_cli_format() {
        let mut config = Config::default();
        config.merge_cli(None, vec![], Some("json".to_string()), None, false);
        assert_eq!(config.output.format, OutputFormat::Json);
    }

    #[test]
    fn test_merge_cli_depth() {
        let mut config = Config::default();
        config.merge_cli(None, vec![], None, Some(15), false);
        assert_eq!(config.analysis.max_depth, 15);
    }

    #[test]
    fn test_merge_cli_no_diagrams() {
        let mut config = Config::default();
        config.merge_cli(None, vec![], None, None, true);
        assert!(!config.diagrams.enabled);
    }

    #[test]
    fn test_output_format_parsing() {
        let toml_str = r#"format = "markdown""#;
        let output: OutputConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(output.format, OutputFormat::Markdown);
    }

    #[test]
    fn test_diagram_layout_parsing() {
        let toml_str = r#"layout = "radial""#;
        let diagram: DiagramConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(diagram.layout, DiagramLayout::Radial);
    }
}
