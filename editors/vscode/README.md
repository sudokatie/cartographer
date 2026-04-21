# Cartographer for VS Code

Architecture documentation and code analysis extension powered by the Cartographer LSP server.

## Features

- **Hover Information**: View module, class, and function details including documentation, dependencies, and complexity metrics
- **Document Symbols**: Navigate to classes, functions, and methods in the current file (Outline view)
- **Workspace Symbols**: Search for symbols across your entire workspace (Ctrl/Cmd+T)
- **Go to Definition**: Jump to symbol definitions, including cross-file navigation
- **Diagnostics**: Real-time warnings for:
  - Circular dependencies (Error)
  - High complexity modules (Warning)
  - Missing documentation (Hint)
- **Code Lens**: View method counts on classes

## Requirements

1. **Cartographer CLI**: Install the Cartographer binary:
   ```bash
   cargo install cartographer
   ```

2. **VS Code**: Version 1.75.0 or higher

## Installation

### From VSIX (Manual)

1. Build the VSIX package:
   ```bash
   cd editors/vscode
   npm install
   npm run package
   ```

2. Install in VS Code:
   - Open VS Code
   - Go to Extensions (Ctrl/Cmd+Shift+X)
   - Click the "..." menu
   - Select "Install from VSIX..."
   - Choose the generated `.vsix` file

### From Source (Development)

1. Clone the repository
2. Open the `editors/vscode` folder in VS Code
3. Run `npm install`
4. Press F5 to launch the Extension Development Host

## Configuration

Configure the extension in your VS Code settings:

```json
{
  // Path to cartographer binary (default: looks in PATH)
  "cartographer.binaryPath": "cartographer",

  // Trace LSP communication (for debugging)
  "cartographer.trace.server": "off",

  // Exclude patterns from analysis
  "cartographer.analysis.exclude": [
    "node_modules/**",
    "target/**",
    "venv/**",
    ".git/**"
  ],

  // Maximum dependency depth
  "cartographer.analysis.maxDepth": 5,

  // Enable/disable diagnostics
  "cartographer.diagnostics.enabled": true,
  "cartographer.diagnostics.circularDependencies": true,
  "cartographer.diagnostics.missingDocs": false,
  "cartographer.diagnostics.complexity": true
}
```

## Commands

- **Cartographer: Analyze Workspace** - Run full analysis and generate documentation
- **Cartographer: Restart Language Server** - Restart the LSP server (useful after configuration changes)

## Supported Languages

- Python (`.py`)
- JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`)
- TypeScript (`.ts`, `.tsx`, `.mts`, `.cts`)
- Rust (`.rs`)
- Go (`.go`)
- Java (`.java`)
- C (`.c`, `.h`)
- C++ (`.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx`)

## Troubleshooting

### Language server not starting

1. Ensure `cartographer` is installed: `cartographer --version`
2. Check the output channel: View > Output > Cartographer
3. Verify the binary path in settings

### No hover information or diagnostics

1. The LSP server analyzes on workspace open. Try restarting the server.
2. Check that your files are in a supported language
3. Look for errors in the Cartographer output channel

### Missing features

Some features require the workspace to be analyzed first. The LSP server analyzes the workspace root on initialization.

## Development

To work on the extension:

```bash
cd editors/vscode
npm install

# Run in development mode
code .
# Press F5 to launch Extension Development Host
```

## License

MIT
