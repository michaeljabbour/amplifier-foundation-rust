# Module Spec: cache

> Layer 2 Component Spec. Implements cache/ module (Wave 1).

## Overview

**Rust module:** `src/cache/` (`mod.rs`, `memory.rs`, `disk.rs`)
**Python source:** `amplifier_foundation/cache/protocol.py` (41 LOC), `simple.py` (50 LOC), `disk.py` (121 LOC)
**Sync/Async:** Fully sync. HashMap ops and std::fs.
**Tests:** 12 tests in `tests/test_cache.rs`
**Internal dependencies:** None for memory cache. Disk cache uses sha2 for key hashing.

## Public API

### cache/mod.rs (CacheProvider trait)

```rust
use crate::bundle::Bundle;

/// Protocol for caching loaded bundles.
/// Foundation provides SimpleCache (in-memory) and DiskCache (filesystem).
pub trait CacheProvider {
    fn get(&self, key: &str) -> Option<&Bundle>;
    fn set(&mut self, key: &str, bundle: Bundle);
    fn clear(&mut self);
    fn contains(&self, key: &str) -> bool;
}
```

**Note:** The trait references `Bundle` which is defined in `bundle/mod.rs`. During Wave 1, Bundle exists as a stub from Wave 0 (F-005). The cache implementations will use this stub. When Bundle is fully implemented in Wave 3, cache implementations update if needed.

### cache/memory.rs

```rust
use crate::bundle::Bundle;
use std::collections::HashMap;

/// Simple in-memory cache. No TTL, no eviction.
/// Bundles cached until clear() or process exit.
pub struct SimpleCache {
    cache: HashMap<String, Bundle>,
}

impl SimpleCache {
    pub fn new() -> Self;
}

impl CacheProvider for SimpleCache { ... }
impl Default for SimpleCache { ... }
```

### cache/disk.rs

```rust
use std::path::{Path, PathBuf};
use crate::bundle::Bundle;

/// Disk-based cache using JSON serialization.
/// Cache directory provided by caller (mechanism, not policy).
pub struct DiskCache {
    cache_dir: PathBuf,
}

impl DiskCache {
    pub fn new(cache_dir: &Path) -> Self;
}

impl CacheProvider for DiskCache { ... }
```

## Translation Decisions

| Python | Rust | Rationale |
|--------|------|-----------|
| `Protocol` class | `trait` | Rust trait = Python protocol |
| `dict[str, Bundle]` | `HashMap<String, Bundle>` | Standard map |
| `__contains__` | `fn contains(&self, key: &str) -> bool` | Explicit method |
| `json.loads(path.read_text())` | `std::fs::read_to_string` + `serde_json::from_str` | Sync I/O |
| `hashlib.sha256(key.encode()).hexdigest()[:16]` | `sha2::Sha256` + hex encoding | Cache key to path |
| `path.write_text(json.dumps(data))` | `serde_json::to_string_pretty` + `std::fs::write` | Sync I/O |
| `missing_ok=True` on unlink | `std::fs::remove_file` with `.ok()` | Ignore missing file |

## Key Behaviors to Preserve

1. **SimpleCache is trivial:** get/set/clear/contains. No TTL, no eviction.
2. **DiskCache key hashing:** SHA-256 of key, first 16 hex chars. Plus first 30 chars of key as safe prefix.
3. **DiskCache safe filename:** Non-alphanumeric chars in key prefix replaced with `_`.
4. **DiskCache auto-creates directory:** `mkdir(parents=True, exist_ok=True)` on init and set.
5. **DiskCache invalid cache recovery:** If JSON parse fails, delete the cache file and return None.
6. **DiskCache serializes Bundle to JSON:** Uses Bundle's fields, converting context paths to strings.

## Notes

- The `CacheProvider` trait uses `Bundle` from `bundle/mod.rs`. In Wave 1, this is a stub. Cache tests that need a real Bundle should construct one from the stub's `Bundle::new()` or `Bundle::from_dict()`.
- DiskCache tests need `tempfile::tempdir()` for isolated filesystem testing.
- The `__contains__` dunder in Python maps to a `contains` method in Rust. We don't implement `std::ops::Index` or similar — just a method.
