# TODOS.md - Cartographer JavaScript/TypeScript Support

## File Tree with Task Status

```
src/
├── analysis/
│   ├── mod.rs           [ ] Add multi-language Analyzer with parser dispatch
│   ├── graph.rs         [x] CodeGraph (reuse for JS/TS)
│   ├── imports.rs       [ ] Extend ImportResolver for JS/TS module resolution
│   ├── metrics.rs       [x] MetricsCalculator (reuse)
│   └── modules.rs       [ ] Extend ModuleDetector for JS/TS patterns
├── cli/
│   ├── mod.rs           [ ] Update messaging from "Python files" to "source files"
│   └── args.rs          [ ] Add --language filter flag
├── parser/
│   ├── mod.rs           [x] Export JavaScriptParser
│   ├── ast.rs           [x] Shared AST types (reuse)
│   ├── python.rs        [x] Python parser (complete)
│   └── javascript.rs    [~] JS/TS parser (needs CommonJS, React, interfaces)
├── output/
│   ├── mod.rs           [x] Output module
│   ├── html.rs          [x] HTML generator (reuse)
│   ├── diagrams.rs      [x] Diagram generator (reuse)
│   └── templates.rs     [x] Template engine (reuse)
├── config.rs            [x] Config struct
├── error.rs             [x] Error types
├── lib.rs               [x] Library exports
└── main.rs              [x] Entry point

tests/
├── integration/
│   └── javascript.rs    [ ] Integration tests for JS/TS analysis
└── fixtures/
    └── javascript/      [ ] JS/TS test fixtures

README.md                [ ] Add JS/TS examples and CLI usage
```

## Legend
- [x] Complete
- [~] In progress / partial
- [ ] Not started

## Priority Order

1. **src/analysis/mod.rs** - Multi-language Analyzer
   - Add Language enum (Python, JavaScript, TypeScript)
   - Add file extension to language detection
   - Dispatch to appropriate parser based on extension
   - Update discover_files() to find JS/TS files

2. **src/cli/mod.rs** - CLI updates
   - Change "Python files" to "source files" or specify by language
   - Show breakdown by language in output

3. **src/parser/javascript.rs** - Parser enhancements
   - Add parse_require() for CommonJS
   - Detect React functional components
   - Parse TypeScript interfaces and type aliases

4. **tests/** - Test coverage
   - Integration test: analyze a real JS/TS project
   - Fixtures: sample JS/TS files for testing

5. **README.md** - Documentation
   - Add JS/TS to feature list
   - Update CLI usage examples
   - Add configuration for JS/TS projects
