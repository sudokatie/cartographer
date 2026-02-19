//! Runtime behavior detection for static analysis limitations.
//!
//! Detects patterns that indicate dynamic behavior that static analysis cannot fully capture:
//! - Dynamic imports (importlib, __import__)
//! - Reflection (getattr, setattr, hasattr, type)
//! - eval/exec usage
//! - Dynamic attribute access (vars, globals, locals)

use std::collections::HashSet;

/// Patterns that indicate dynamic/runtime behavior.
#[derive(Debug, Clone, Default)]
pub struct DynamicPatterns {
    /// Dynamic import calls found.
    pub dynamic_imports: Vec<DynamicImport>,
    /// Reflection usage found.
    pub reflection_calls: Vec<ReflectionCall>,
    /// eval/exec calls found.
    pub eval_exec_calls: Vec<EvalExecCall>,
    /// Dynamic attribute access found.
    pub dynamic_attrs: Vec<DynamicAttrAccess>,
}

impl DynamicPatterns {
    /// Create empty patterns.
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if any dynamic patterns were found.
    pub fn has_dynamic_behavior(&self) -> bool {
        !self.dynamic_imports.is_empty()
            || !self.reflection_calls.is_empty()
            || !self.eval_exec_calls.is_empty()
            || !self.dynamic_attrs.is_empty()
    }
    
    /// Get a risk score for dynamic behavior (0-100).
    /// Higher scores indicate more dynamic/harder to analyze code.
    pub fn risk_score(&self) -> u32 {
        let mut score = 0u32;
        
        // eval/exec is highest risk (arbitrary code execution)
        score += self.eval_exec_calls.len() as u32 * 25;
        
        // Dynamic imports are moderate risk
        score += self.dynamic_imports.len() as u32 * 15;
        
        // Reflection is moderate risk
        score += self.reflection_calls.len() as u32 * 10;
        
        // Dynamic attrs are lower risk
        score += self.dynamic_attrs.len() as u32 * 5;
        
        score.min(100)
    }
    
    /// Merge another DynamicPatterns into this one.
    pub fn merge(&mut self, other: DynamicPatterns) {
        self.dynamic_imports.extend(other.dynamic_imports);
        self.reflection_calls.extend(other.reflection_calls);
        self.eval_exec_calls.extend(other.eval_exec_calls);
        self.dynamic_attrs.extend(other.dynamic_attrs);
    }
}

/// A dynamic import call.
#[derive(Debug, Clone)]
pub struct DynamicImport {
    /// The function used (importlib.import_module, __import__, etc.)
    pub function: String,
    /// Line number.
    pub line: usize,
    /// The module being imported (if statically determinable).
    pub target: Option<String>,
}

/// A reflection call.
#[derive(Debug, Clone)]
pub struct ReflectionCall {
    /// The reflection function (getattr, setattr, hasattr, type, etc.)
    pub function: String,
    /// Line number.
    pub line: usize,
}

/// An eval or exec call.
#[derive(Debug, Clone)]
pub struct EvalExecCall {
    /// "eval" or "exec"
    pub function: String,
    /// Line number.
    pub line: usize,
}

/// Dynamic attribute access.
#[derive(Debug, Clone)]
pub struct DynamicAttrAccess {
    /// The function (vars, globals, locals, __dict__)
    pub function: String,
    /// Line number.
    pub line: usize,
}

/// Detect dynamic patterns in Python source code.
pub fn detect_python_dynamic_patterns(source: &str) -> DynamicPatterns {
    let mut patterns = DynamicPatterns::new();
    
    // Known dynamic import functions (kept for reference, manual checks below)
    let _dynamic_import_funcs: HashSet<&str> = 
        ["__import__", "importlib.import_module", "import_module"].into_iter().collect();
    
    // Known reflection functions
    let reflection_funcs: HashSet<&str> = 
        ["getattr", "setattr", "hasattr", "delattr", "type", "isinstance", "issubclass", 
         "callable", "dir", "inspect.getmembers", "inspect.getattr_static"].into_iter().collect();
    
    // eval/exec functions
    let eval_exec_funcs: HashSet<&str> = ["eval", "exec", "compile"].into_iter().collect();
    
    // Dynamic attribute access
    let dynamic_attr_funcs: HashSet<&str> = 
        ["vars", "globals", "locals", "__dict__", "__getattr__", "__setattr__"].into_iter().collect();
    
    for (line_num, line) in source.lines().enumerate() {
        let line_num = line_num + 1; // 1-indexed
        let trimmed = line.trim();
        
        // Skip comments and empty lines
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        
        // Check for dynamic imports (check longest match first to avoid duplicates)
        let mut found_import = false;
        if line.contains("importlib.import_module") && line.contains('(') {
            patterns.dynamic_imports.push(DynamicImport {
                function: "importlib.import_module".to_string(),
                line: line_num,
                target: extract_string_arg(line),
            });
            found_import = true;
        }
        if !found_import && line.contains("__import__") && line.contains('(') {
            patterns.dynamic_imports.push(DynamicImport {
                function: "__import__".to_string(),
                line: line_num,
                target: extract_string_arg(line),
            });
        }
        
        // Check for reflection
        for func in &reflection_funcs {
            // Match function calls, not just substring matches
            if contains_function_call(line, func) {
                patterns.reflection_calls.push(ReflectionCall {
                    function: func.to_string(),
                    line: line_num,
                });
            }
        }
        
        // Check for eval/exec
        for func in &eval_exec_funcs {
            if contains_function_call(line, func) {
                patterns.eval_exec_calls.push(EvalExecCall {
                    function: func.to_string(),
                    line: line_num,
                });
            }
        }
        
        // Check for dynamic attr access
        for func in &dynamic_attr_funcs {
            if contains_function_call(line, func) || line.contains(&format!(".{}", func)) {
                patterns.dynamic_attrs.push(DynamicAttrAccess {
                    function: func.to_string(),
                    line: line_num,
                });
            }
        }
    }
    
    patterns
}

/// Check if a line contains a function call (not just the name).
fn contains_function_call(line: &str, func: &str) -> bool {
    // Look for all occurrences of func followed by (
    let mut search_start = 0;
    while let Some(rel_pos) = line[search_start..].find(func) {
        let pos = search_start + rel_pos;
        let after = &line[pos + func.len()..];
        if after.starts_with('(') {
            // Make sure it's not part of a larger word
            if pos == 0 {
                return true;
            }
            let before = line.chars().nth(pos - 1).unwrap_or(' ');
            if !before.is_alphanumeric() && before != '_' {
                return true;
            }
        }
        search_start = pos + 1;
    }
    false
}

/// Try to extract a string argument from a function call.
fn extract_string_arg(line: &str) -> Option<String> {
    // Look for quoted string in the line
    if let Some(start) = line.find('"') {
        if let Some(end) = line[start + 1..].find('"') {
            return Some(line[start + 1..start + 1 + end].to_string());
        }
    }
    if let Some(start) = line.find('\'') {
        if let Some(end) = line[start + 1..].find('\'') {
            return Some(line[start + 1..start + 1 + end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_dynamic_import() {
        let source = r#"
import importlib
mod = importlib.import_module("mymodule")
"#;
        let patterns = detect_python_dynamic_patterns(source);
        assert_eq!(patterns.dynamic_imports.len(), 1);
        assert_eq!(patterns.dynamic_imports[0].function, "importlib.import_module");
        assert_eq!(patterns.dynamic_imports[0].target, Some("mymodule".to_string()));
    }
    
    #[test]
    fn test_detect_dunder_import() {
        let source = r#"
mod = __import__("os")
"#;
        let patterns = detect_python_dynamic_patterns(source);
        assert_eq!(patterns.dynamic_imports.len(), 1);
        assert_eq!(patterns.dynamic_imports[0].function, "__import__");
    }
    
    #[test]
    fn test_detect_getattr() {
        let source = r#"
value = getattr(obj, "attr_name")
setattr(obj, "other", 123)
"#;
        let patterns = detect_python_dynamic_patterns(source);
        assert_eq!(patterns.reflection_calls.len(), 2);
    }
    
    #[test]
    fn test_detect_eval_exec() {
        let source = r#"
result = eval(user_input)
exec(code_string)
"#;
        let patterns = detect_python_dynamic_patterns(source);
        assert_eq!(patterns.eval_exec_calls.len(), 2);
        assert!(patterns.eval_exec_calls.iter().any(|c| c.function == "eval"));
        assert!(patterns.eval_exec_calls.iter().any(|c| c.function == "exec"));
    }
    
    #[test]
    fn test_detect_vars_globals() {
        let source = r#"
local_vars = vars()
global_vars = globals()
"#;
        let patterns = detect_python_dynamic_patterns(source);
        assert_eq!(patterns.dynamic_attrs.len(), 2);
    }
    
    #[test]
    fn test_risk_score() {
        // eval/exec should have high risk
        let source = "eval(x)\nexec(y)";
        let patterns = detect_python_dynamic_patterns(source);
        assert!(patterns.risk_score() >= 50);
    }
    
    #[test]
    fn test_no_dynamic_behavior() {
        let source = r#"
def regular_function():
    return 42

class NormalClass:
    pass
"#;
        let patterns = detect_python_dynamic_patterns(source);
        assert!(!patterns.has_dynamic_behavior());
        assert_eq!(patterns.risk_score(), 0);
    }
    
    #[test]
    fn test_skip_comments() {
        let source = r#"
# eval(dangerous_code)  -- this is a comment
# getattr(obj, "attr")
"#;
        let patterns = detect_python_dynamic_patterns(source);
        assert!(!patterns.has_dynamic_behavior());
    }
    
    #[test]
    fn test_merge_patterns() {
        let mut p1 = DynamicPatterns::new();
        p1.eval_exec_calls.push(EvalExecCall { function: "eval".into(), line: 1 });
        
        let mut p2 = DynamicPatterns::new();
        p2.reflection_calls.push(ReflectionCall { function: "getattr".into(), line: 5 });
        
        p1.merge(p2);
        assert_eq!(p1.eval_exec_calls.len(), 1);
        assert_eq!(p1.reflection_calls.len(), 1);
    }
    
    #[test]
    fn test_function_call_detection() {
        // Should match actual function calls
        assert!(contains_function_call("getattr(obj, 'x')", "getattr"));
        assert!(contains_function_call("x = eval(y)", "eval"));
        
        // Should not match substrings
        assert!(!contains_function_call("my_getattr = 5", "getattr"));
        assert!(!contains_function_call("evaluate(x)", "eval"));
    }
}
