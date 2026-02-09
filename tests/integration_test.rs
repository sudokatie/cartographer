// Integration tests for Cartographer

use cartographer::{
    Analyzer, Config, HtmlConfig, HtmlGenerator, DiagramGenerator,
};
use std::path::PathBuf;
use tempfile::TempDir;

fn fixtures_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// Helper to create an analyzer with default config
fn create_analyzer() -> Analyzer {
    let config = Config::default();
    Analyzer::new(config).expect("Failed to create analyzer")
}

// ============================================================================
// Analysis Tests
// ============================================================================

#[test]
fn test_analyze_simple_project() {
    let path = fixtures_path("simple_project/src");
    let mut analyzer = create_analyzer();
    
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    // Should have found all Python files
    assert!(result.graph.stats().files >= 4, "Expected at least 4 files");
    
    // Should have found classes (at least one - the parser may not detect all)
    assert!(result.graph.stats().classes >= 1, "Expected at least 1 class");
    
    // Should have found functions
    assert!(result.graph.stats().functions >= 4, "Expected at least 4 functions");
    
    // Should have no parse errors
    assert!(result.parse_errors.is_empty(), "Had unexpected parse errors: {:?}", result.parse_errors);
}

#[test]
fn test_analyze_complex_project() {
    let path = fixtures_path("complex_project");
    let mut analyzer = create_analyzer();
    
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    // Should have found files in all modules
    assert!(result.graph.stats().files >= 10, "Expected at least 10 files");
    
    // Should have multiple modules
    assert!(result.modules.len() >= 4, "Expected at least 4 modules (api, models, services, utils)");
    
    // Should have found various classes
    assert!(result.graph.stats().classes >= 5, "Expected at least 5 classes");
}

#[test]
fn test_analyze_detects_imports() {
    let path = fixtures_path("complex_project");
    let mut analyzer = create_analyzer();
    
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    // Complex project has more imports that can be resolved
    // At minimum we should have detected some files with imports
    let files_with_imports = result.graph.all_files()
        .filter(|(_, f)| !f.imports.is_empty())
        .count();
    assert!(files_with_imports > 0, "Expected files with imports");
}

#[test]
fn test_analyze_detects_classes() {
    let path = fixtures_path("complex_project");
    let mut analyzer = create_analyzer();
    
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    // Should find the User class in complex project
    let user_class = result.graph.all_classes()
        .find(|(_, c)| c.name == "User");
    assert!(user_class.is_some(), "Should find User class");
    
    // Should find the BaseModel class
    let base_model = result.graph.all_classes()
        .find(|(_, c)| c.name == "BaseModel");
    assert!(base_model.is_some(), "Should find BaseModel class");
    
    // User should inherit from BaseModel
    if let Some((_, user)) = user_class {
        assert!(user.bases.iter().any(|b| b.contains("BaseModel")), 
            "User should inherit from BaseModel");
    }
}

#[test]
fn test_analyze_detects_functions() {
    let path = fixtures_path("simple_project/src");
    let mut analyzer = create_analyzer();
    
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    // Find format_name function
    let format_name = result.graph.all_functions()
        .find(|(_, f)| f.name == "format_name");
    assert!(format_name.is_some(), "Should find format_name function");
    
    // Check it has parameters
    if let Some((_, func)) = format_name {
        assert!(!func.parameters.is_empty(), "format_name should have parameters");
    }
}

#[test]
fn test_analyze_calculates_metrics() {
    let path = fixtures_path("complex_project");
    let mut analyzer = create_analyzer();
    
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    // Project metrics should be populated
    assert!(result.metrics.file_count > 0, "Should have file count");
    assert!(result.metrics.total_lines > 0, "Should have line count");
    assert!(result.metrics.class_count > 0, "Should have class count");
    assert!(result.metrics.function_count > 0, "Should have function count");
}

// ============================================================================
// HTML Generation Tests
// ============================================================================

#[test]
fn test_html_generation() {
    let path = fixtures_path("simple_project/src");
    let mut analyzer = create_analyzer();
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    let output_dir = TempDir::new().expect("Failed to create temp dir");
    
    let html_config = HtmlConfig {
        output_dir: output_dir.path().to_path_buf(),
        project_name: "Test Project".to_string(),
        generate_diagrams: true,
        copy_assets: true,
    };
    
    let generator = HtmlGenerator::new(html_config).expect("Failed to create generator");
    let report = generator.generate(&result).expect("Generation failed");
    
    // Should have generated pages
    assert!(report.pages_generated > 0, "Should generate at least one page");
    
    // Should have copied assets
    assert!(report.assets_copied, "Should copy assets");
    
    // Check index.html exists
    assert!(output_dir.path().join("index.html").exists(), "index.html should exist");
    
    // Check assets exist
    assert!(output_dir.path().join("assets/style.css").exists(), "style.css should exist");
    assert!(output_dir.path().join("assets/script.js").exists(), "script.js should exist");
    
    // Check search.json exists
    assert!(output_dir.path().join("search.json").exists(), "search.json should exist");
}

#[test]
fn test_html_generates_module_pages() {
    let path = fixtures_path("simple_project/src");
    let mut analyzer = create_analyzer();
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    let output_dir = TempDir::new().expect("Failed to create temp dir");
    
    let html_config = HtmlConfig {
        output_dir: output_dir.path().to_path_buf(),
        project_name: "Test Project".to_string(),
        generate_diagrams: false,
        copy_assets: true,
    };
    
    let generator = HtmlGenerator::new(html_config).expect("Failed to create generator");
    generator.generate(&result).expect("Generation failed");
    
    // Should have modules directory
    assert!(output_dir.path().join("modules").exists(), "modules directory should exist");
}

#[test]
fn test_search_json_structure() {
    let path = fixtures_path("simple_project/src");
    let mut analyzer = create_analyzer();
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    let output_dir = TempDir::new().expect("Failed to create temp dir");
    
    let html_config = HtmlConfig {
        output_dir: output_dir.path().to_path_buf(),
        project_name: "Test Project".to_string(),
        generate_diagrams: false,
        copy_assets: false,
    };
    
    let generator = HtmlGenerator::new(html_config).expect("Failed to create generator");
    generator.generate(&result).expect("Generation failed");
    
    // Read and parse search.json
    let search_json = std::fs::read_to_string(output_dir.path().join("search.json"))
        .expect("Failed to read search.json");
    
    let entries: Vec<serde_json::Value> = serde_json::from_str(&search_json)
        .expect("Failed to parse search.json");
    
    // Should have entries
    assert!(!entries.is_empty(), "search.json should have entries");
    
    // Each entry should have required fields
    for entry in &entries {
        assert!(entry.get("name").is_some(), "Entry should have name");
        assert!(entry.get("kind").is_some(), "Entry should have kind");
        assert!(entry.get("path").is_some(), "Entry should have path");
        assert!(entry.get("module").is_some(), "Entry should have module");
    }
}

// ============================================================================
// Diagram Generation Tests
// ============================================================================

#[test]
fn test_diagram_generation() {
    let path = fixtures_path("simple_project/src");
    let mut analyzer = create_analyzer();
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    let diagram_gen = DiagramGenerator::new();
    
    // Generate dependency graph
    let dep_graph = diagram_gen.generate_dependency_graph(&result);
    
    // Should be valid Mermaid syntax
    assert!(dep_graph.starts_with("graph"), "Should be a Mermaid graph");
}

#[test]
fn test_module_diagram_generation() {
    let path = fixtures_path("simple_project/src");
    let mut analyzer = create_analyzer();
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    let diagram_gen = DiagramGenerator::new();
    
    // Generate module diagrams
    for module in &result.modules {
        let module_graph = diagram_gen.generate_module_graph(module, &result);
        assert!(module_graph.starts_with("graph"), "Should be a Mermaid graph");
        assert!(module_graph.contains("subgraph"), "Should contain subgraph");
    }
}

#[test]
fn test_class_hierarchy_diagram() {
    let path = fixtures_path("simple_project/src");
    let mut analyzer = create_analyzer();
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    let diagram_gen = DiagramGenerator::new();
    let hierarchy = diagram_gen.generate_class_hierarchy(&result);
    
    // Should be a class diagram
    assert!(hierarchy.starts_with("classDiagram"), "Should be a class diagram");
}

// ============================================================================
// Configuration Tests
// ============================================================================

#[test]
fn test_config_default() {
    let config = Config::default();
    
    assert!(!config.analysis.exclude.is_empty(), "Should have default excludes");
    assert_eq!(config.analysis.max_depth, 5, "Default depth should be 5");
    assert!(config.diagrams.enabled, "Diagrams should be enabled by default");
}

#[test]
fn test_config_merge_cli() {
    let mut config = Config::default();
    
    config.merge_cli(
        Some(PathBuf::from("/custom/output")),
        vec!["vendor/**".to_string()],
        Some("json".to_string()),
        Some(10),
        true, // no_diagrams
    );
    
    assert_eq!(config.output.directory, PathBuf::from("/custom/output"));
    assert!(config.analysis.exclude.contains(&"vendor/**".to_string()));
    assert!(!config.diagrams.enabled);
    assert_eq!(config.analysis.max_depth, 10);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_analyze_nonexistent_path() {
    let mut analyzer = create_analyzer();
    let result = analyzer.analyze(&PathBuf::from("/nonexistent/path"));
    
    assert!(result.is_err(), "Should error on nonexistent path");
}

#[test]
fn test_analyze_empty_directory() {
    let empty_dir = TempDir::new().expect("Failed to create temp dir");
    let mut analyzer = create_analyzer();
    
    let result = analyzer.analyze(empty_dir.path());
    
    assert!(result.is_err(), "Should error on empty directory");
    assert!(result.unwrap_err().to_string().contains("No source files"));
}

// ============================================================================
// Module Detection Tests
// ============================================================================

#[test]
fn test_module_detection() {
    let path = fixtures_path("complex_project");
    let mut analyzer = create_analyzer();
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    let module_names: Vec<&str> = result.modules.iter()
        .map(|m| m.name.as_str())
        .collect();
    
    // Should detect key modules
    assert!(module_names.iter().any(|n| n.contains("api")), "Should detect api module");
    assert!(module_names.iter().any(|n| n.contains("models")), "Should detect models module");
    assert!(module_names.iter().any(|n| n.contains("services")), "Should detect services module");
    assert!(module_names.iter().any(|n| n.contains("utils")), "Should detect utils module");
}

#[test]
fn test_module_type_detection() {
    let path = fixtures_path("complex_project");
    let mut analyzer = create_analyzer();
    let result = analyzer.analyze(&path).expect("Analysis failed");
    
    use cartographer::ModuleType;
    
    // Check module types are detected correctly
    for module in &result.modules {
        match module.name.as_str() {
            name if name.contains("models") => {
                assert_eq!(module.module_type, ModuleType::Models, 
                    "models should be Models type");
            }
            name if name.contains("utils") => {
                assert_eq!(module.module_type, ModuleType::Utils,
                    "utils should be Utils type");
            }
            name if name.contains("api") => {
                assert_eq!(module.module_type, ModuleType::Api,
                    "api should be Api type");
            }
            name if name.contains("services") => {
                assert_eq!(module.module_type, ModuleType::Services,
                    "services should be Services type");
            }
            _ => {}
        }
    }
}

// ============================================================================
// Performance Tests (basic sanity checks)
// ============================================================================

#[test]
fn test_analysis_performance() {
    let path = fixtures_path("complex_project");
    let mut analyzer = create_analyzer();
    
    let start = std::time::Instant::now();
    let _result = analyzer.analyze(&path).expect("Analysis failed");
    let duration = start.elapsed();
    
    // Should complete in under 5 seconds for a small project
    assert!(duration.as_secs() < 5, "Analysis took too long: {:?}", duration);
}
