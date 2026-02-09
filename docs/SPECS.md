# SPECS.md - Cartographer JavaScript/TypeScript Support

## Feature Overview
Add JavaScript and TypeScript support to Cartographer, enabling analysis of JS/TS codebases with the same quality as Python analysis.

## Acceptance Criteria (from FEATURE-BACKLOG.md)
- [x] tree-sitter-javascript and tree-sitter-typescript parsers
- [~] Extract: imports, exports, classes, functions, React components
- [ ] Detect module patterns (CommonJS, ESM)
- [x] Handle JSX/TSX
- [ ] Generate dependency graphs for JS/TS
- [ ] Tests for JS/TS parsing
- [ ] Update README and examples

## Technical Specifications

### 1. Language Detection
File extensions:
- `.js`, `.mjs`, `.cjs` → JavaScript
- `.jsx` → JavaScript with JSX
- `.ts`, `.mts`, `.cts` → TypeScript
- `.tsx` → TypeScript with JSX

### 2. Parser Capabilities

#### ESM Imports (Complete)
```javascript
import foo from 'module';           // Default import
import { foo, bar } from 'module';  // Named imports
import * as mod from 'module';      // Namespace import
import './module';                  // Side-effect import
```

#### CommonJS (TODO)
```javascript
const foo = require('module');
const { bar } = require('module');
module.exports = foo;
exports.bar = bar;
```

#### Classes (Complete)
```javascript
class MyClass extends BaseClass {
    constructor(props) {}
    method() {}
    static staticMethod() {}
}
```

#### Functions (Complete)
```javascript
function named() {}
const arrow = () => {};
async function asyncFn() {}
const asyncArrow = async () => {};
```

#### React Components (TODO)
```javascript
// Functional component detection
function MyComponent(props) { return <div />; }
const MyComponent = () => <div />;
const MyComponent = React.memo(() => <div />);
const MyComponent = forwardRef((props, ref) => <div />);
```

#### TypeScript Types (TODO)
```typescript
interface User { name: string; }
type Status = 'active' | 'inactive';
enum Color { Red, Green, Blue }
```

### 3. Analyzer Integration

The Analyzer needs to:
1. Detect language from file extension
2. Use appropriate parser (PythonParser or JavaScriptParser)
3. Build unified CodeGraph across languages
4. Resolve imports for each language's module system

### 4. Import Resolution

JavaScript module resolution:
- Relative: `./module`, `../module`
- Package: `lodash`, `@scope/package`
- Node built-ins: `fs`, `path`
- Index files: `./dir` → `./dir/index.js`

### 5. Output Changes

No changes needed to output generators - they work on CodeGraph which is language-agnostic.

## Code Locations

| Component | File | Status |
|-----------|------|--------|
| JS Parser | src/parser/javascript.rs | 70% |
| Analyzer | src/analysis/mod.rs | Needs multi-lang |
| CLI | src/cli/mod.rs | Needs update |
| Tests | tests/ | Needs JS tests |
| Docs | README.md | Needs update |

## Testing Strategy

1. Unit tests for each parse function in javascript.rs
2. Integration test: analyze a sample JS/TS project
3. Cross-language test: mixed Python + JS project
4. Edge cases: dynamic imports, re-exports, barrel files
