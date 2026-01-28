// Import resolution for Python modules
//
// Resolves imports to determine if they are:
// - Standard library (don't follow)
// - Third-party (don't follow)
// - Local (follow and include in graph)

use crate::parser::{Import, ImportKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Classification of an import
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportType {
    /// Python standard library module
    Stdlib,
    /// Third-party package (from site-packages)
    ThirdParty,
    /// Local module within the project
    Local,
    /// Unknown/unresolved import
    Unknown,
}

/// Result of resolving an import
#[derive(Debug, Clone)]
pub struct ResolvedImport {
    /// The original import
    pub import: Import,
    /// Classification
    pub import_type: ImportType,
    /// Resolved file path (for local imports)
    pub resolved_path: Option<PathBuf>,
    /// Module name after resolution
    pub resolved_module: String,
}

/// Resolves Python imports to their sources
pub struct ImportResolver {
    /// Project root directory
    project_root: PathBuf,
    /// Set of known stdlib module names
    stdlib_modules: HashSet<String>,
    /// Set of known third-party module names (from requirements, etc.)
    third_party_modules: HashSet<String>,
}

impl ImportResolver {
    /// Create a new import resolver for a project
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            stdlib_modules: Self::load_stdlib_modules(),
            third_party_modules: HashSet::new(),
        }
    }

    /// Add known third-party modules (e.g., from requirements.txt)
    pub fn add_third_party(&mut self, modules: impl IntoIterator<Item = String>) {
        self.third_party_modules.extend(modules);
    }

    /// Load the set of Python standard library module names
    fn load_stdlib_modules() -> HashSet<String> {
        // Python 3.10+ stdlib modules (top-level only)
        // This list covers the most common modules
        let stdlib = [
            // Built-in and core
            "abc", "aifc", "argparse", "array", "ast", "asynchat", "asyncio",
            "asyncore", "atexit", "audioop", "base64", "bdb", "binascii",
            "binhex", "bisect", "builtins", "bz2", "calendar", "cgi", "cgitb",
            "chunk", "cmath", "cmd", "code", "codecs", "codeop", "collections",
            "colorsys", "compileall", "concurrent", "configparser", "contextlib",
            "contextvars", "copy", "copyreg", "cProfile", "crypt", "csv",
            "ctypes", "curses", "dataclasses", "datetime", "dbm", "decimal",
            "difflib", "dis", "distutils", "doctest", "email", "encodings",
            "enum", "errno", "faulthandler", "fcntl", "filecmp", "fileinput",
            "fnmatch", "fractions", "ftplib", "functools", "gc", "getopt",
            "getpass", "gettext", "glob", "graphlib", "grp", "gzip", "hashlib",
            "heapq", "hmac", "html", "http", "idlelib", "imaplib", "imghdr",
            "imp", "importlib", "inspect", "io", "ipaddress", "itertools",
            "json", "keyword", "lib2to3", "linecache", "locale", "logging",
            "lzma", "mailbox", "mailcap", "marshal", "math", "mimetypes",
            "mmap", "modulefinder", "multiprocessing", "netrc", "nis",
            "nntplib", "numbers", "operator", "optparse", "os", "ossaudiodev",
            "pathlib", "pdb", "pickle", "pickletools", "pipes", "pkgutil",
            "platform", "plistlib", "poplib", "posix", "posixpath", "pprint",
            "profile", "pstats", "pty", "pwd", "py_compile", "pyclbr",
            "pydoc", "queue", "quopri", "random", "re", "readline", "reprlib",
            "resource", "rlcompleter", "runpy", "sched", "secrets", "select",
            "selectors", "shelve", "shlex", "shutil", "signal", "site",
            "smtpd", "smtplib", "sndhdr", "socket", "socketserver", "spwd",
            "sqlite3", "ssl", "stat", "statistics", "string", "stringprep",
            "struct", "subprocess", "sunau", "symtable", "sys", "sysconfig",
            "syslog", "tabnanny", "tarfile", "telnetlib", "tempfile", "termios",
            "test", "textwrap", "threading", "time", "timeit", "tkinter",
            "token", "tokenize", "tomllib", "trace", "traceback", "tracemalloc",
            "tty", "turtle", "turtledemo", "types", "typing", "unicodedata",
            "unittest", "urllib", "uu", "uuid", "venv", "warnings", "wave",
            "weakref", "webbrowser", "winreg", "winsound", "wsgiref", "xdrlib",
            "xml", "xmlrpc", "zipapp", "zipfile", "zipimport", "zlib",
            // Common typing extensions
            "typing_extensions",
            // Underscore modules
            "_thread", "__future__",
        ];
        stdlib.iter().map(|s| s.to_string()).collect()
    }

    /// Check if a module name is from the standard library
    pub fn is_stdlib(&self, module: &str) -> bool {
        // Get the top-level module name
        let top_level = module.split('.').next().unwrap_or(module);
        self.stdlib_modules.contains(top_level)
    }

    /// Check if a module name is a known third-party package
    pub fn is_third_party(&self, module: &str) -> bool {
        let top_level = module.split('.').next().unwrap_or(module);
        self.third_party_modules.contains(top_level)
    }

    /// Resolve an import to its source
    pub fn resolve(&self, import: &Import, current_file: &Path) -> ResolvedImport {
        let module = &import.module;
        
        // Handle relative imports
        if let ImportKind::Relative { level } = &import.kind {
            return self.resolve_relative(import, current_file, *level);
        }

        // Check stdlib first
        if self.is_stdlib(module) {
            return ResolvedImport {
                import: import.clone(),
                import_type: ImportType::Stdlib,
                resolved_path: None,
                resolved_module: module.clone(),
            };
        }

        // Check known third-party
        if self.is_third_party(module) {
            return ResolvedImport {
                import: import.clone(),
                import_type: ImportType::ThirdParty,
                resolved_path: None,
                resolved_module: module.clone(),
            };
        }

        // Try to resolve as local import
        if let Some(path) = self.find_local_module(module) {
            return ResolvedImport {
                import: import.clone(),
                import_type: ImportType::Local,
                resolved_path: Some(path),
                resolved_module: module.clone(),
            };
        }

        // If not found locally, assume third-party
        ResolvedImport {
            import: import.clone(),
            import_type: ImportType::ThirdParty,
            resolved_path: None,
            resolved_module: module.clone(),
        }
    }

    /// Resolve a relative import (e.g., from ..utils import helper)
    fn resolve_relative(&self, import: &Import, current_file: &Path, level: usize) -> ResolvedImport {
        // Get the directory of the current file
        let current_dir = current_file.parent().unwrap_or(Path::new(""));
        
        // Go up 'level' directories (level=1 means current package, level=2 means parent, etc.)
        let mut base_dir = current_dir.to_path_buf();
        for _ in 1..level {
            if let Some(parent) = base_dir.parent() {
                base_dir = parent.to_path_buf();
            }
        }

        // Construct the full module path
        let full_module = if import.module.is_empty() {
            // from . import x or from .. import x
            self.path_to_module(&base_dir)
        } else {
            // from .utils import x or from ..utils import x
            let base_module = self.path_to_module(&base_dir);
            if base_module.is_empty() {
                import.module.clone()
            } else {
                format!("{}.{}", base_module, import.module)
            }
        };

        // Try to find the module file
        let module_path = if import.module.is_empty() {
            base_dir.clone()
        } else {
            base_dir.join(import.module.replace('.', "/"))
        };

        // Check for module file or package
        let resolved_path = self.find_module_file(&module_path);

        ResolvedImport {
            import: import.clone(),
            import_type: if resolved_path.is_some() {
                ImportType::Local
            } else {
                ImportType::Unknown
            },
            resolved_path,
            resolved_module: full_module,
        }
    }

    /// Convert a file path to a module name
    fn path_to_module(&self, path: &Path) -> String {
        let relative = path.strip_prefix(&self.project_root).unwrap_or(path);
        relative
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join(".")
    }

    /// Find a local module by name
    fn find_local_module(&self, module: &str) -> Option<PathBuf> {
        // Convert module.name to path
        let module_path = self.project_root.join(module.replace('.', "/"));
        self.find_module_file(&module_path)
    }

    /// Find the actual file for a module path
    fn find_module_file(&self, module_path: &Path) -> Option<PathBuf> {
        // Try as a direct .py file
        let py_file = module_path.with_extension("py");
        if py_file.exists() {
            return Some(py_file);
        }

        // Try as a package (directory with __init__.py)
        let init_file = module_path.join("__init__.py");
        if init_file.exists() {
            return Some(init_file);
        }

        None
    }

    /// Resolve all imports in a file
    pub fn resolve_all(&self, imports: &[Import], current_file: &Path) -> Vec<ResolvedImport> {
        imports
            .iter()
            .map(|imp| self.resolve(imp, current_file))
            .collect()
    }

    /// Get all local imports from a list of resolved imports
    pub fn local_imports(resolved: &[ResolvedImport]) -> Vec<&ResolvedImport> {
        resolved
            .iter()
            .filter(|r| r.import_type == ImportType::Local)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ImportedName;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        
        // Create a simple project structure:
        // project/
        //   src/
        //     __init__.py
        //     main.py
        //     utils/
        //       __init__.py
        //       helpers.py
        //   tests/
        //     test_main.py
        
        let src = dir.path().join("src");
        let utils = src.join("utils");
        let tests = dir.path().join("tests");
        
        fs::create_dir_all(&utils).unwrap();
        fs::create_dir_all(&tests).unwrap();
        
        fs::write(src.join("__init__.py"), "").unwrap();
        fs::write(src.join("main.py"), "# main").unwrap();
        fs::write(utils.join("__init__.py"), "").unwrap();
        fs::write(utils.join("helpers.py"), "# helpers").unwrap();
        fs::write(tests.join("test_main.py"), "# tests").unwrap();
        
        dir
    }

    #[test]
    fn test_stdlib_detection() {
        let resolver = ImportResolver::new(PathBuf::from("/tmp"));
        
        assert!(resolver.is_stdlib("os"));
        assert!(resolver.is_stdlib("sys"));
        assert!(resolver.is_stdlib("json"));
        assert!(resolver.is_stdlib("collections"));
        assert!(resolver.is_stdlib("typing"));
        assert!(resolver.is_stdlib("pathlib"));
        
        // Submodules
        assert!(resolver.is_stdlib("os.path"));
        assert!(resolver.is_stdlib("collections.abc"));
        
        // Not stdlib
        assert!(!resolver.is_stdlib("numpy"));
        assert!(!resolver.is_stdlib("requests"));
        assert!(!resolver.is_stdlib("django"));
    }

    #[test]
    fn test_third_party_detection() {
        let mut resolver = ImportResolver::new(PathBuf::from("/tmp"));
        resolver.add_third_party(vec!["numpy".to_string(), "requests".to_string()]);
        
        assert!(resolver.is_third_party("numpy"));
        assert!(resolver.is_third_party("numpy.array"));
        assert!(resolver.is_third_party("requests"));
        
        assert!(!resolver.is_third_party("os"));
        assert!(!resolver.is_third_party("mymodule"));
    }

    #[test]
    fn test_resolve_stdlib_import() {
        let resolver = ImportResolver::new(PathBuf::from("/tmp"));
        
        let import = Import::simple("os", 1);
        let resolved = resolver.resolve(&import, Path::new("/tmp/test.py"));
        
        assert_eq!(resolved.import_type, ImportType::Stdlib);
        assert!(resolved.resolved_path.is_none());
    }

    #[test]
    fn test_resolve_local_import() {
        let project = create_test_project();
        let resolver = ImportResolver::new(project.path().to_path_buf());
        
        let import = Import::simple("src.main", 1);
        let resolved = resolver.resolve(&import, project.path().join("test.py").as_path());
        
        assert_eq!(resolved.import_type, ImportType::Local);
        assert!(resolved.resolved_path.is_some());
    }

    #[test]
    fn test_resolve_relative_import() {
        let project = create_test_project();
        let resolver = ImportResolver::new(project.path().to_path_buf());
        
        // from .utils import helpers (from src/main.py)
        let import = Import::relative(
            "utils",
            vec![ImportedName::new("helpers")],
            1,
            1,
        );
        let current_file = project.path().join("src/main.py");
        let resolved = resolver.resolve(&import, &current_file);
        
        assert_eq!(resolved.import_type, ImportType::Local);
    }

    #[test]
    fn test_resolve_unknown_defaults_to_third_party() {
        let resolver = ImportResolver::new(PathBuf::from("/tmp"));
        
        // Unknown module that doesn't exist locally
        let import = Import::simple("some_unknown_package", 1);
        let resolved = resolver.resolve(&import, Path::new("/tmp/test.py"));
        
        // Unknown modules are assumed to be third-party
        assert_eq!(resolved.import_type, ImportType::ThirdParty);
    }

    #[test]
    fn test_local_imports_filter() {
        let resolved = vec![
            ResolvedImport {
                import: Import::simple("os", 1),
                import_type: ImportType::Stdlib,
                resolved_path: None,
                resolved_module: "os".to_string(),
            },
            ResolvedImport {
                import: Import::simple("mymodule", 2),
                import_type: ImportType::Local,
                resolved_path: Some(PathBuf::from("/project/mymodule.py")),
                resolved_module: "mymodule".to_string(),
            },
            ResolvedImport {
                import: Import::simple("requests", 3),
                import_type: ImportType::ThirdParty,
                resolved_path: None,
                resolved_module: "requests".to_string(),
            },
        ];
        
        let local = ImportResolver::local_imports(&resolved);
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].resolved_module, "mymodule");
    }
}
