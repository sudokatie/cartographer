//! LLM-based code explanation generation.
//!
//! Generates natural language explanations for modules, classes, and functions
//! using configurable LLM providers (Ollama, OpenAI).

use crate::config::{LlmConfig, LlmProvider};
use crate::error::{Error, Result};
use crate::parser::ast::{Class, Function, ParsedFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// Cache for LLM explanations to avoid regenerating
#[derive(Debug, Default)]
pub struct ExplanationCache {
    cache: Mutex<HashMap<String, String>>,
}

impl ExplanationCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.cache.lock().ok()?.get(key).cloned()
    }

    pub fn set(&self, key: String, value: String) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key, value);
        }
    }

    pub fn len(&self) -> usize {
        self.cache.lock().map(|c| c.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Generates explanations for code elements
pub struct ExplanationGenerator {
    config: LlmConfig,
    cache: ExplanationCache,
    client: reqwest::blocking::Client,
}

/// Response from Ollama API
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

/// Response from OpenAI API
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    content: String,
}

/// Request to OpenAI API
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIRequestMessage>,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct OpenAIRequestMessage {
    role: String,
    content: String,
}

impl ExplanationGenerator {
    /// Create a new explanation generator
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            cache: ExplanationCache::new(),
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Check if LLM explanations are enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Generate an explanation for a module/file
    pub fn explain_module(&self, file: &ParsedFile) -> Result<String> {
        if !self.config.enabled {
            return Ok(self.template_module_explanation(file));
        }

        let cache_key = format!("module:{}", file.path.display());
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let prompt = self.build_module_prompt(file);
        match self.query_llm(&prompt) {
            Ok(explanation) => {
                self.cache.set(cache_key, explanation.clone());
                Ok(explanation)
            }
            Err(_) => Ok(self.template_module_explanation(file)),
        }
    }

    /// Generate an explanation for a class
    pub fn explain_class(&self, class: &Class, file_path: &str) -> Result<String> {
        if !self.config.enabled {
            return Ok(self.template_class_explanation(class));
        }

        let cache_key = format!("class:{}:{}", file_path, class.name);
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let prompt = self.build_class_prompt(class, file_path);
        match self.query_llm(&prompt) {
            Ok(explanation) => {
                self.cache.set(cache_key, explanation.clone());
                Ok(explanation)
            }
            Err(_) => Ok(self.template_class_explanation(class)),
        }
    }

    /// Generate an explanation for a function
    pub fn explain_function(&self, func: &Function, context: &str) -> Result<String> {
        if !self.config.enabled {
            return Ok(self.template_function_explanation(func));
        }

        let cache_key = format!("function:{}:{}", context, func.name);
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let prompt = self.build_function_prompt(func, context);
        match self.query_llm(&prompt) {
            Ok(explanation) => {
                self.cache.set(cache_key, explanation.clone());
                Ok(explanation)
            }
            Err(_) => Ok(self.template_function_explanation(func)),
        }
    }

    /// Query the configured LLM
    fn query_llm(&self, prompt: &str) -> Result<String> {
        match self.config.provider {
            LlmProvider::Ollama => self.query_ollama(prompt),
            LlmProvider::OpenAI => self.query_openai(prompt),
        }
    }

    fn query_ollama(&self, prompt: &str) -> Result<String> {
        let url = self
            .config
            .api_url
            .as_deref()
            .unwrap_or("http://localhost:11434");
        let endpoint = format!("{}/api/generate", url);

        let body = serde_json::json!({
            "model": self.config.model,
            "prompt": prompt,
            "stream": false
        });

        let response = self
            .client
            .post(&endpoint)
            .json(&body)
            .send()
            .map_err(|e| Error::llm(format!("Ollama request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::llm(format!(
                "Ollama returned status {}",
                response.status()
            )));
        }

        let result: OllamaResponse = response
            .json()
            .map_err(|e| Error::llm(format!("Failed to parse Ollama response: {}", e)))?;

        Ok(result.response.trim().to_string())
    }

    fn query_openai(&self, prompt: &str) -> Result<String> {
        let url = self
            .config
            .api_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1");
        let endpoint = format!("{}/chat/completions", url);

        let api_key = self
            .config
            .api_key
            .clone()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| Error::llm("OpenAI API key not configured"))?;

        let request = OpenAIRequest {
            model: self.config.model.clone(),
            messages: vec![OpenAIRequestMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: 500,
        };

        let response = self
            .client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&request)
            .send()
            .map_err(|e| Error::llm(format!("OpenAI request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::llm(format!(
                "OpenAI returned status {}",
                response.status()
            )));
        }

        let result: OpenAIResponse = response
            .json()
            .map_err(|e| Error::llm(format!("Failed to parse OpenAI response: {}", e)))?;

        result
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .ok_or_else(|| Error::llm("No response from OpenAI"))
    }

    fn build_module_prompt(&self, file: &ParsedFile) -> String {
        let classes: Vec<_> = file.classes.iter().map(|c| c.name.as_str()).collect();
        let functions: Vec<_> = file.functions.iter().map(|f| f.name.as_str()).collect();
        let imports: Vec<_> = file.imports.iter().map(|i| i.module.as_str()).collect();

        format!(
            "Explain this Python module in 2-3 sentences. Be concise and technical.\n\n\
            File: {}\n\
            Classes: {}\n\
            Functions: {}\n\
            Imports: {}\n\n\
            Explanation:",
            file.path.display(),
            if classes.is_empty() {
                "none".to_string()
            } else {
                classes.join(", ")
            },
            if functions.is_empty() {
                "none".to_string()
            } else {
                functions.join(", ")
            },
            if imports.is_empty() {
                "none".to_string()
            } else {
                imports.join(", ")
            }
        )
    }

    fn build_class_prompt(&self, class: &Class, file_path: &str) -> String {
        let methods: Vec<_> = class.methods.iter().map(|m| m.name.as_str()).collect();
        let bases = if class.bases.is_empty() {
            "none".to_string()
        } else {
            class.bases.join(", ")
        };

        format!(
            "Explain this Python class in 2-3 sentences. Be concise and technical.\n\n\
            Class: {}\n\
            File: {}\n\
            Bases: {}\n\
            Methods: {}\n\
            Docstring: {}\n\n\
            Explanation:",
            class.name,
            file_path,
            bases,
            if methods.is_empty() {
                "none".to_string()
            } else {
                methods.join(", ")
            },
            class.docstring.as_deref().unwrap_or("none")
        )
    }

    fn build_function_prompt(&self, func: &Function, context: &str) -> String {
        let params: Vec<_> = func
            .parameters
            .iter()
            .map(|p| {
                if let Some(ref t) = p.type_hint {
                    format!("{}: {}", p.name, t)
                } else {
                    p.name.clone()
                }
            })
            .collect();

        format!(
            "Explain this Python function in 1-2 sentences. Be concise and technical.\n\n\
            Function: {}\n\
            Context: {}\n\
            Parameters: {}\n\
            Returns: {}\n\
            Docstring: {}\n\
            Is async: {}\n\n\
            Explanation:",
            func.name,
            context,
            if params.is_empty() {
                "none".to_string()
            } else {
                params.join(", ")
            },
            func.return_type.as_deref().unwrap_or("unspecified"),
            func.docstring.as_deref().unwrap_or("none"),
            func.is_async
        )
    }

    /// Template-based explanation when LLM is unavailable
    fn template_module_explanation(&self, file: &ParsedFile) -> String {
        let class_count = file.classes.len();
        let func_count = file.functions.len();

        if class_count == 0 && func_count == 0 {
            "This module contains only imports and constants.".to_string()
        } else if class_count > 0 && func_count > 0 {
            format!(
                "This module defines {} class{} and {} function{}.",
                class_count,
                if class_count == 1 { "" } else { "es" },
                func_count,
                if func_count == 1 { "" } else { "s" }
            )
        } else if class_count > 0 {
            format!(
                "This module defines {} class{}.",
                class_count,
                if class_count == 1 { "" } else { "es" }
            )
        } else {
            format!(
                "This module defines {} function{}.",
                func_count,
                if func_count == 1 { "" } else { "s" }
            )
        }
    }

    fn template_class_explanation(&self, class: &Class) -> String {
        let method_count = class.methods.len();
        let base_info = if class.bases.is_empty() {
            String::new()
        } else {
            format!(" It inherits from {}.", class.bases.join(", "))
        };

        format!(
            "The {} class has {} method{}.{}",
            class.name,
            method_count,
            if method_count == 1 { "" } else { "s" },
            base_info
        )
    }

    fn template_function_explanation(&self, func: &Function) -> String {
        let param_count = func.parameters.len();
        let async_prefix = if func.is_async { "async " } else { "" };
        let return_info = func
            .return_type
            .as_ref()
            .map(|t| format!(" Returns {}.", t))
            .unwrap_or_default();

        format!(
            "The {} is an {}function that takes {} parameter{}.{}",
            func.name,
            async_prefix,
            param_count,
            if param_count == 1 { "" } else { "s" },
            return_info
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{Import, Parameter, ParameterKind};
    use std::path::PathBuf;

    fn make_test_config(enabled: bool) -> LlmConfig {
        LlmConfig {
            enabled,
            provider: LlmProvider::Ollama,
            model: "test".to_string(),
            api_url: None,
            api_key: None,
        }
    }

    fn make_test_file() -> ParsedFile {
        let mut file = ParsedFile::new(PathBuf::from("test.py"), "test".to_string());
        file.imports.push(Import::simple("os", 1));
        
        let mut class = Class::new("TestClass", 5);
        class.bases = vec!["BaseClass".to_string()];
        class.docstring = Some("A test class".to_string());
        class.line_end = 10;
        file.classes.push(class);
        
        let mut func = Function::new("test_func", 12);
        func.parameters.push(Parameter {
            name: "x".to_string(),
            type_hint: Some("int".to_string()),
            default: None,
            kind: ParameterKind::Regular,
        });
        func.return_type = Some("str".to_string());
        func.docstring = Some("Test function".to_string());
        func.line_end = 15;
        file.functions.push(func);
        
        file
    }

    #[test]
    fn test_cache_operations() {
        let cache = ExplanationCache::new();
        assert!(cache.is_empty());

        cache.set("key1".to_string(), "value1".to_string());
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("key1"), Some("value1".to_string()));
        assert_eq!(cache.get("missing"), None);
    }

    #[test]
    fn test_template_module_explanation() {
        let config = make_test_config(false);
        let generator = ExplanationGenerator::new(config);
        let file = make_test_file();

        let explanation = generator.template_module_explanation(&file);
        assert!(explanation.contains("1 class"));
        assert!(explanation.contains("1 function"));
    }

    #[test]
    fn test_template_class_explanation() {
        let config = make_test_config(false);
        let generator = ExplanationGenerator::new(config);

        let mut class = Class::new("MyClass", 1);
        class.bases = vec!["Parent".to_string()];
        class.line_end = 5;

        let explanation = generator.template_class_explanation(&class);
        assert!(explanation.contains("MyClass"));
        assert!(explanation.contains("inherits from Parent"));
    }

    #[test]
    fn test_template_function_explanation() {
        let config = make_test_config(false);
        let generator = ExplanationGenerator::new(config);

        let mut func = Function::new("do_thing", 1);
        func.parameters = vec![
            Parameter {
                name: "a".to_string(),
                type_hint: None,
                default: None,
                kind: ParameterKind::Regular,
            },
            Parameter {
                name: "b".to_string(),
                type_hint: None,
                default: None,
                kind: ParameterKind::Regular,
            },
        ];
        func.return_type = Some("bool".to_string());
        func.is_async = true;
        func.line_end = 3;

        let explanation = generator.template_function_explanation(&func);
        assert!(explanation.contains("do_thing"));
        assert!(explanation.contains("async"));
        assert!(explanation.contains("2 parameters"));
        assert!(explanation.contains("Returns bool"));
    }

    #[test]
    fn test_explain_module_disabled() {
        let config = make_test_config(false);
        let generator = ExplanationGenerator::new(config);
        let file = make_test_file();

        let result = generator.explain_module(&file);
        assert!(result.is_ok());
        // Should use template, not LLM
        let explanation = result.unwrap();
        assert!(explanation.contains("class"));
    }

    #[test]
    fn test_explain_class_disabled() {
        let config = make_test_config(false);
        let generator = ExplanationGenerator::new(config);

        let mut class = Class::new("Example", 1);
        class.line_end = 5;

        let result = generator.explain_class(&class, "example.py");
        assert!(result.is_ok());
    }

    #[test]
    fn test_explain_function_disabled() {
        let config = make_test_config(false);
        let generator = ExplanationGenerator::new(config);

        let mut func = Function::new("helper", 1);
        func.line_end = 2;

        let result = generator.explain_function(&func, "module.py");
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_enabled() {
        let enabled_config = make_test_config(true);
        let disabled_config = make_test_config(false);

        assert!(ExplanationGenerator::new(enabled_config).is_enabled());
        assert!(!ExplanationGenerator::new(disabled_config).is_enabled());
    }

    #[test]
    fn test_build_module_prompt() {
        let config = make_test_config(true);
        let generator = ExplanationGenerator::new(config);
        let file = make_test_file();

        let prompt = generator.build_module_prompt(&file);
        assert!(prompt.contains("test.py"));
        assert!(prompt.contains("TestClass"));
        assert!(prompt.contains("test_func"));
    }

    #[test]
    fn test_build_class_prompt() {
        let config = make_test_config(true);
        let generator = ExplanationGenerator::new(config);

        let mut class = Class::new("DataProcessor", 1);
        class.bases = vec!["BaseProcessor".to_string()];
        class.docstring = Some("Processes data".to_string());
        class.line_end = 10;
        
        let mut method = Function::new("process", 3);
        method.line_end = 5;
        class.methods.push(method);

        let prompt = generator.build_class_prompt(&class, "processor.py");
        assert!(prompt.contains("DataProcessor"));
        assert!(prompt.contains("BaseProcessor"));
        assert!(prompt.contains("process"));
        assert!(prompt.contains("Processes data"));
    }

    #[test]
    fn test_build_function_prompt() {
        let config = make_test_config(true);
        let generator = ExplanationGenerator::new(config);

        let mut func = Function::new("calculate", 1);
        func.parameters = vec![
            Parameter {
                name: "x".to_string(),
                type_hint: Some("int".to_string()),
                default: None,
                kind: ParameterKind::Regular,
            },
            Parameter {
                name: "y".to_string(),
                type_hint: Some("int".to_string()),
                default: None,
                kind: ParameterKind::Regular,
            },
        ];
        func.return_type = Some("int".to_string());
        func.docstring = Some("Calculate sum".to_string());
        func.line_end = 3;

        let prompt = generator.build_function_prompt(&func, "math.py");
        assert!(prompt.contains("calculate"));
        assert!(prompt.contains("x: int"));
        assert!(prompt.contains("y: int"));
        assert!(prompt.contains("int")); // return type
        assert!(prompt.contains("Calculate sum"));
    }
}
