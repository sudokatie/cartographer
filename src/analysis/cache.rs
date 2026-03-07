//! Incremental analysis cache
//!
//! Caches parse results keyed by content hash to avoid re-parsing unchanged files.

use crate::error::{Error, Result};
use crate::parser::ParsedFile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Cache entry for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// SHA-256 hash of the file content
    pub content_hash: String,
    /// Last modified time (for quick invalidation check)
    pub mtime: u64,
    /// The parsed file data
    pub parsed: ParsedFile,
}

/// Incremental analysis cache
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisCache {
    /// Cache version for format compatibility
    version: u32,
    /// Hash of the config used for this cache
    config_hash: String,
    /// Root path this cache was created for
    root_path: PathBuf,
    /// Cached entries keyed by relative path
    entries: HashMap<PathBuf, CacheEntry>,
}

const CACHE_VERSION: u32 = 1;
const CACHE_FILENAME: &str = ".cartographer-cache.json";

impl AnalysisCache {
    /// Create a new empty cache
    pub fn new(root: &Path, config_hash: &str) -> Self {
        Self {
            version: CACHE_VERSION,
            config_hash: config_hash.to_string(),
            root_path: root.to_path_buf(),
            entries: HashMap::new(),
        }
    }

    /// Load cache from disk if it exists and is valid
    pub fn load(root: &Path, config_hash: &str) -> Option<Self> {
        let cache_path = root.join(CACHE_FILENAME);
        
        let content = fs::read_to_string(&cache_path).ok()?;
        let cache: Self = serde_json::from_str(&content).ok()?;
        
        // Validate cache
        if cache.version != CACHE_VERSION {
            return None;
        }
        
        if cache.config_hash != config_hash {
            return None;
        }
        
        if cache.root_path != root {
            return None;
        }
        
        Some(cache)
    }

    /// Save cache to disk
    pub fn save(&self, root: &Path) -> Result<()> {
        let cache_path = root.join(CACHE_FILENAME);
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| Error::Analysis(format!("Failed to serialize cache: {}", e)))?;
        
        fs::write(&cache_path, content).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to write cache file: {}", e),
            ))
        })?;
        
        Ok(())
    }

    /// Check if a file is cached and unchanged
    pub fn is_valid(&self, path: &Path, root: &Path) -> bool {
        let relative = match path.strip_prefix(root) {
            Ok(r) => r.to_path_buf(),
            Err(_) => return false,
        };

        let entry = match self.entries.get(&relative) {
            Some(e) => e,
            None => return false,
        };

        // Quick check: mtime
        let mtime = match get_mtime(path) {
            Some(m) => m,
            None => return false,
        };

        // If mtime is the same, cache is valid (fast path)
        if mtime == entry.mtime {
            return true;
        }

        // mtime changed - verify content hash to confirm
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return false,
        };
        
        let hash = hash_content(&content);
        hash == entry.content_hash
    }

    /// Get cached parsed file if valid
    pub fn get(&self, path: &Path, root: &Path) -> Option<&ParsedFile> {
        if self.is_valid(path, root) {
            let relative = path.strip_prefix(root).ok()?;
            self.entries.get(relative).map(|e| &e.parsed)
        } else {
            None
        }
    }

    /// Store a parsed file in the cache
    pub fn put(&mut self, path: &Path, root: &Path, parsed: ParsedFile) -> Result<()> {
        let relative = path.strip_prefix(root).map_err(|_| {
            Error::Analysis(format!("Path {} is not under root {}", path.display(), root.display()))
        })?.to_path_buf();

        let content = fs::read_to_string(path).map_err(Error::Io)?;
        let content_hash = hash_content(&content);
        let mtime = get_mtime(path).unwrap_or(0);

        self.entries.insert(
            relative,
            CacheEntry {
                content_hash,
                mtime,
                parsed,
            },
        );

        Ok(())
    }

    /// Remove stale entries for files that no longer exist
    pub fn prune(&mut self, root: &Path) {
        self.entries.retain(|relative, _| {
            let path = root.join(relative);
            path.exists()
        });
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len(),
            version: self.version,
        }
    }
}

/// Statistics about the cache
#[derive(Debug)]
pub struct CacheStats {
    pub entries: usize,
    pub version: u32,
}

/// Compute SHA-256 hash of content
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Hash a config for cache invalidation
pub fn hash_config(config: &crate::config::Config) -> String {
    // Serialize config and hash it
    let content = serde_json::to_string(config).unwrap_or_default();
    hash_content(&content)
}

/// Get file modification time as unix timestamp
fn get_mtime(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config() -> crate::config::Config {
        crate::config::Config::default()
    }

    #[test]
    fn test_hash_content() {
        let hash1 = hash_content("hello world");
        let hash2 = hash_content("hello world");
        let hash3 = hash_content("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA-256 hex length
    }

    #[test]
    fn test_hash_config() {
        let config1 = create_test_config();
        let config2 = create_test_config();
        let mut config3 = create_test_config();
        config3.analysis.exclude.push("extra/**".to_string());

        let hash1 = hash_config(&config1);
        let hash2 = hash_config(&config2);
        let hash3 = hash_config(&config3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_cache_new() {
        let root = Path::new("/tmp/project");
        let cache = AnalysisCache::new(root, "abc123");

        assert_eq!(cache.version, CACHE_VERSION);
        assert_eq!(cache.config_hash, "abc123");
        assert_eq!(cache.root_path, root);
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn test_cache_save_and_load() {
        let dir = TempDir::new().unwrap();
        let config = create_test_config();
        let config_hash = hash_config(&config);

        // Create and save cache
        let cache = AnalysisCache::new(dir.path(), &config_hash);
        cache.save(dir.path()).unwrap();

        // Load cache
        let loaded = AnalysisCache::load(dir.path(), &config_hash);
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.version, CACHE_VERSION);
        assert_eq!(loaded.config_hash, config_hash);
    }

    #[test]
    fn test_cache_invalidation_on_config_change() {
        let dir = TempDir::new().unwrap();
        let config = create_test_config();
        let config_hash = hash_config(&config);

        // Save cache with original config
        let cache = AnalysisCache::new(dir.path(), &config_hash);
        cache.save(dir.path()).unwrap();

        // Try to load with different config hash
        let loaded = AnalysisCache::load(dir.path(), "different_hash");
        assert!(loaded.is_none());
    }

    #[test]
    fn test_cache_put_and_get() {
        let dir = TempDir::new().unwrap();
        let config = create_test_config();
        let config_hash = hash_config(&config);

        // Create a test file
        let file_path = dir.path().join("test.py");
        fs::write(&file_path, "def hello(): pass").unwrap();

        // Create parsed file
        let parsed = ParsedFile::new(file_path.clone(), "test".to_string());

        // Put in cache
        let mut cache = AnalysisCache::new(dir.path(), &config_hash);
        cache.put(&file_path, dir.path(), parsed.clone()).unwrap();

        // Get from cache
        let retrieved = cache.get(&file_path, dir.path());
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().module_name, "test");
    }

    #[test]
    fn test_cache_invalidation_on_content_change() {
        let dir = TempDir::new().unwrap();
        let config = create_test_config();
        let config_hash = hash_config(&config);

        // Create a test file
        let file_path = dir.path().join("test.py");
        fs::write(&file_path, "def hello(): pass").unwrap();

        // Create parsed file and cache it
        let parsed = ParsedFile::new(file_path.clone(), "test".to_string());

        let mut cache = AnalysisCache::new(dir.path(), &config_hash);
        cache.put(&file_path, dir.path(), parsed).unwrap();

        // Modify the file with different content
        fs::write(&file_path, "def goodbye(): pass").unwrap();

        // Manually set an old mtime in the cache entry to force hash check
        let relative = file_path.strip_prefix(dir.path()).unwrap().to_path_buf();
        if let Some(entry) = cache.entries.get_mut(&relative) {
            entry.mtime = 0; // Force mtime mismatch
        }

        // Cache should be invalid (content hash differs)
        assert!(!cache.is_valid(&file_path, dir.path()));
        assert!(cache.get(&file_path, dir.path()).is_none());
    }

    #[test]
    fn test_cache_prune() {
        let dir = TempDir::new().unwrap();
        let config = create_test_config();
        let config_hash = hash_config(&config);

        // Create test files
        let file1 = dir.path().join("keep.py");
        let file2 = dir.path().join("delete.py");
        fs::write(&file1, "x = 1").unwrap();
        fs::write(&file2, "y = 2").unwrap();

        // Cache both files
        let parsed1 = ParsedFile::new(file1.clone(), "keep".to_string());

        let mut cache = AnalysisCache::new(dir.path(), &config_hash);
        cache.put(&file1, dir.path(), parsed1).unwrap();
        
        let parsed2 = ParsedFile::new(file2.clone(), "delete".to_string());
        cache.put(&file2, dir.path(), parsed2).unwrap();

        assert_eq!(cache.stats().entries, 2);

        // Delete one file
        fs::remove_file(&file2).unwrap();

        // Prune cache
        cache.prune(dir.path());

        assert_eq!(cache.stats().entries, 1);
        assert!(cache.get(&file1, dir.path()).is_some());
    }

    #[test]
    fn test_cache_stats() {
        let dir = TempDir::new().unwrap();
        let cache = AnalysisCache::new(dir.path(), "test");

        let stats = cache.stats();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.version, CACHE_VERSION);
    }
}
