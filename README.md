# Cartographer

Generate architecture documentation from codebases. Point at any repo, get a map.

## Why?

Every codebase has documentation that was accurate two years ago, scattered comments, and tribal knowledge locked in the heads of developers who've already left. New developers spend weeks figuring out where things are.

Cartographer reads your code and generates living documentation - architecture diagrams, module explanations, dependency graphs. Documentation that can't drift because it's derived from source.

## Features

- **Multi-language support**: Python, JavaScript, TypeScript (including JSX/TSX)
- **React component detection**: Automatically identifies React functional components
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
- [ ] Rust support
- [ ] Go support

### v0.3 (Planned)
- [ ] LLM-generated explanations
- [ ] Runtime behavior detection hints

## License

MIT

## Author

Katie

---

*The tool I wish existed every time I inherit a codebase.*
