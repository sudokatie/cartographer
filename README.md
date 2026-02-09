# Cartographer

Generate architecture documentation from codebases. Point at any repo, get a map.

## Why?

Every codebase has documentation that was accurate two years ago, scattered comments, and tribal knowledge locked in the heads of developers who've already left. New developers spend weeks figuring out where things are.

Cartographer reads your code and generates living documentation - architecture diagrams, module explanations, dependency graphs. Documentation that can't drift because it's derived from source.

## Features

- **Multi-language support**: Python, JavaScript, TypeScript, Rust, Go
- **React component detection**: Automatically identifies React functional components
- **Rust support**: Parses structs, enums, traits, impl blocks, use statements
- **Go support**: Parses packages, structs, interfaces, methods, imports
- **Dependency graphs**: See what imports what, find circular dependencies
- **Module detection**: Group files into logical components
- **Beautiful output**: Static HTML site you can deploy anywhere
- **Metrics**: Lines of code, class count, import statistics

## Quick Start

```bash
# Install
cargo install cartographer

# Analyze a project (Python, JS, or TS)
cartographer analyze ./my-project

# Serve the generated docs
cartographer serve ./cartographer-docs
```

## Usage

```
cartographer analyze <path> [options]

Options:
  -o, --output <dir>     Output directory (default: ./cartographer-docs)
  --exclude <patterns>   Glob patterns to exclude (repeatable)
  --include <patterns>   Glob patterns to include
  -c, --config <file>    Config file path (default: cartographer.toml)
  --format <type>        Output format: html, json, markdown
  --depth <n>            Max dependency depth (default: 5)
  --no-diagrams          Skip diagram generation
  -v, --verbose          Verbose output
```

## Supported Languages

| Language | Extensions | Features |
|----------|------------|----------|
| Python | `.py` | Classes, functions, imports, decorators |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` | ESM imports, CommonJS require, classes, React components |
| TypeScript | `.ts`, `.tsx`, `.mts`, `.cts` | Same as JS + type annotations |
| Rust | `.rs` | Structs, enums, traits, impl blocks, use statements, const/static |
| Go | `.go` | Packages, structs, interfaces, methods, functions, const/var |

### JavaScript/TypeScript Examples

```bash
# Analyze a React project
cartographer analyze ./my-react-app

# Analyze a Node.js backend
cartographer analyze ./api-server --exclude "node_modules/**"

# Mixed Python + JS monorepo
cartographer analyze ./monorepo
```

Cartographer automatically detects:
- ESM imports (`import { foo } from './module'`)
- CommonJS requires (`const foo = require('./module')`)
- React functional components (PascalCase functions returning JSX)

### Rust Examples

```bash
# Analyze a Rust project
cartographer analyze ./my-rust-project --exclude "target/**"

# Analyze a Cargo workspace
cartographer analyze ./workspace

# Cartographer analyzing itself
cartographer analyze .
```

Cartographer extracts from Rust code:
- Structs, enums, and traits
- Impl blocks (methods associated with types)
- Use statements and module declarations
- Const and static items
- Async functions
- Doc comments

### Go Examples

```bash
# Analyze a Go project
cartographer analyze ./my-go-project --exclude "vendor/**"

# Analyze a Go module
cartographer analyze ./cmd/myapp
```

Cartographer extracts from Go code:
- Packages and imports
- Structs and interfaces
- Methods with receiver types
- Functions with variadic parameters
- Const and var declarations
- Comments

## Configuration

Create `cartographer.toml` in your project root:

```toml
[project]
name = "My Project"
description = "What it does"

[analysis]
exclude = ["tests/**", "node_modules/**", "venv/**"]
max_depth = 5

[output]
format = "html"
directory = "./docs"
```

## Roadmap

### v0.2 (Current)
- [x] JavaScript/TypeScript support
- [x] React component detection
- [x] Rust support
- [x] Go support

### v0.3 (Planned)
- [ ] LLM-generated explanations
- [ ] Runtime behavior detection hints

## License

MIT

## Author

Katie

---

*The tool I wish existed every time I inherit a codebase.*
