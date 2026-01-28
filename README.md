# Cartographer

Generate architecture documentation from codebases. Point at any repo, get a map.

## Why?

Every codebase has documentation that was accurate two years ago, scattered comments, and tribal knowledge locked in the heads of developers who've already left. New developers spend weeks figuring out where things are.

Cartographer reads your code and generates living documentation - architecture diagrams, module explanations, dependency graphs. Documentation that can't drift because it's derived from source.

## Features

- **Automatic analysis**: Parse Python codebases, extract structure
- **Dependency graphs**: See what imports what, find circular dependencies
- **Module detection**: Group files into logical components
- **Beautiful output**: Static HTML site you can deploy anywhere
- **Metrics**: Lines of code, class count, import statistics

## Quick Start

```bash
# Install
cargo install cartographer

# Analyze a project
cartographer analyze ./my-python-project

# Serve the generated docs
cartographer serve ./cartographer-docs
```

## Usage

```
cartographer analyze <path> [options]

Options:
  -o, --output <dir>     Output directory (default: ./cartographer-docs)
  --exclude <patterns>   Glob patterns to exclude (repeatable)
  --include <patterns>   Glob patterns to include (default: **/*.py)
  -c, --config <file>    Config file path (default: cartographer.toml)
  --format <type>        Output format: html, json, markdown
  --depth <n>            Max dependency depth (default: 5)
  --no-diagrams          Skip diagram generation
  -v, --verbose          Verbose output
```

## Configuration

Create `cartographer.toml` in your project root:

```toml
[project]
name = "My Project"
description = "What it does"

[analysis]
exclude = ["tests/**", "venv/**"]
max_depth = 5

[output]
format = "html"
directory = "./docs"
```

## Current Limitations

- Python only (more languages planned)
- No LLM-generated explanations (template-based for now)
- Static analysis only (won't detect runtime behavior)

## License

MIT

## Author

Katie the Clawdius Prime

---

*The tool I wish existed every time I inherit a codebase.*
