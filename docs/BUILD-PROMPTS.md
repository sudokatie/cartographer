# BUILD-PROMPTS.md - Cartographer JavaScript/TypeScript Support

## Coding Standards

### Rust Style
- Use `thiserror` for error types
- Use `Result<T>` alias from error module
- Follow existing patterns in codebase
- All public items need doc comments
- Run `cargo fmt` and `cargo clippy` before committing

### Parser Conventions
- Use tree-sitter for all parsing
- Return `Option<T>` for optional parse results
- Collect errors but don't fail entire analysis on parse error
- Use helper functions for repeated patterns

### Testing
- Unit tests in `#[cfg(test)]` module at bottom of file
- Integration tests in `tests/` directory
- Use `tempfile` for filesystem tests
- Name tests descriptively: `test_parse_esm_import`

## Implementation Order

### Step 1: Multi-Language Analyzer
Modify `src/analysis/mod.rs`:

```rust
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "py" => Some(Self::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            _ => None,
        }
    }
}
```

Add to Analyzer struct:
```rust
pub struct Analyzer {
    config: Config,
    python_parser: PythonParser,
    js_parser: JavaScriptParser,
    verbose: bool,
}
```

Update `discover_files()` to find all supported extensions.
Update `parse_and_build_graph()` to dispatch by language.

### Step 2: CommonJS Support
Add to `src/parser/javascript.rs`:

```rust
fn parse_require(node: &Node, source: &[u8]) -> Option<Import> {
    // Look for: require('module') or require("module")
    // Handle destructuring: const { foo } = require('module')
}
```

Add case to `visit_node()` for `call_expression` where callee is `require`.

### Step 3: React Component Detection
Add to AST or use Class/Function with `is_component` flag:

```rust
fn is_react_component(func: &Function, source: &str) -> bool {
    // Check if function:
    // 1. Name starts with uppercase
    // 2. Returns JSX (has jsx_element in body)
    // 3. Uses hooks (useState, useEffect, etc.)
}
```

### Step 4: TypeScript Types
Parse interfaces and type aliases:

```rust
fn parse_interface(node: &Node, source: &[u8]) -> Option<Interface> {
    // Extract interface name, extends, properties
}

fn parse_type_alias(node: &Node, source: &[u8]) -> Option<TypeAlias> {
    // Extract type name and definition
}
```

May need new AST types for TypeScript-specific constructs.

### Step 5: Update CLI
Change messages in `src/cli/mod.rs`:
- "Found {} Python files" → "Found {} source files ({} Python, {} JavaScript, {} TypeScript)"
- Add `--language` filter flag to args.rs

### Step 6: Tests
Create `tests/integration/javascript.rs`:
- Test analyzing a pure JS project
- Test analyzing a TS project with interfaces
- Test mixed Python + JS project

Create fixtures in `tests/fixtures/javascript/`:
- sample-project/ with realistic JS structure
- react-app/ with React components
- typescript/ with interfaces and types

### Step 7: README
Update with:
- JavaScript/TypeScript in feature list
- CLI examples for JS/TS projects
- Configuration example for JS/TS

## Commit Messages
- feat: add multi-language support to analyzer
- feat: add CommonJS require parsing
- feat: detect React functional components
- feat: parse TypeScript interfaces
- test: add JavaScript integration tests
- docs: update readme with JS/TS support
