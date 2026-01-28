# Changelog

All notable changes to Cartographer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-01-28

Initial release. Cartographer analyzes Python codebases and generates architecture documentation.

### Added

- **Python Parser**: Full parsing of Python source files using tree-sitter
  - Modules, imports, classes, functions, constants
  - Type hints, docstrings, decorators
  - Async functions, relative imports

- **Code Analysis**
  - Dependency graph with import tracking
  - Circular dependency detection
  - Transitive import analysis (configurable depth)
  - Module detection (models, views, services, utils, api, tests, config, core)
  - Metrics: LOC, class/function counts, import breakdown, public/private ratio

- **Documentation Generation**
  - Static HTML site output
  - Index page with project overview and search
  - Module pages with file listings and metrics
  - Class pages with methods and inheritance
  - Mermaid diagrams for dependencies

- **CLI Interface**
  - `cartographer analyze <path>` - analyze a codebase
  - `cartographer serve <output>` - serve generated docs
  - `cartographer version` - show version
  - Options: --output, --exclude, --include, --config, --format, --depth, --no-diagrams, --verbose

- **Configuration**: TOML config file support with CLI overrides

- **UI Features**
  - Dark/light mode toggle
  - Client-side search
  - Responsive design
  - Collapsible sections
  - Code block copy button

### Technical Details

- Built with Rust for performance
- tree-sitter-python for reliable parsing
- Tera templates for HTML generation
- 137 tests (118 unit + 19 integration)

---

*The tool I wish existed every time I inherit a codebase.*
