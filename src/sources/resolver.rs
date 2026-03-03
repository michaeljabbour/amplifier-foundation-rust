use std::path::PathBuf;

use crate::paths::uri::{parse_uri, ResolvedSource};

use async_trait::async_trait;

use super::file::FileSourceHandler;
use super::git::GitSourceHandler;
use super::http::HttpSourceHandler;
use super::zip::ZipSourceHandler;
use super::{SourceHandler, SourceResolver};

/// Simple implementation of source resolution.
///
/// Supports:
/// - `file://` and local paths via [`FileSourceHandler`]
/// - `git+https://` via [`GitSourceHandler`]
/// - `zip+https://` and `zip+file://` via [`ZipSourceHandler`]
/// - `https://` and `http://` via [`HttpSourceHandler`]
///
/// Handlers are tried in order; first match wins.
/// Custom handlers can be added via [`add_handler`](Self::add_handler)
/// and take priority over defaults.
pub struct SimpleSourceResolver {
    handlers: Vec<Box<dyn SourceHandler>>,
    cache_dir: PathBuf,
    /// Base path used for resolving relative file URIs.
    /// Stored for parity with Python's `self.base_path`.
    #[allow(dead_code)]
    base_path: PathBuf,
}

impl Default for SimpleSourceResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleSourceResolver {
    /// Create a resolver with default handlers and default cache/base paths.
    pub fn new() -> Self {
        let base_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let cache_dir = crate::paths::uri::get_amplifier_home()
            .join("cache")
            .join("bundles");

        Self {
            handlers: Self::default_handlers(base_path.clone()),
            cache_dir,
            base_path,
        }
    }

    /// Create a resolver with an explicit base path for file resolution.
    pub fn with_base_path(base_path: PathBuf) -> Self {
        let cache_dir = crate::paths::uri::get_amplifier_home()
            .join("cache")
            .join("bundles");

        Self {
            handlers: Self::default_handlers(base_path.clone()),
            cache_dir,
            base_path,
        }
    }

    /// Create a resolver with an explicit cache directory.
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        let base_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        Self {
            handlers: Self::default_handlers(base_path.clone()),
            cache_dir,
            base_path,
        }
    }

    /// Build the default handler chain.
    ///
    /// Order matters for URI matching:
    /// 1. File — handles `file://` and local paths
    /// 2. Git — handles `git+https://`
    /// 3. Zip — handles `zip+*://` (must be before Http so `zip+https` doesn't
    ///    fall through to the plain-https handler)
    /// 4. Http — handles `https://` and `http://`
    fn default_handlers(base_path: PathBuf) -> Vec<Box<dyn SourceHandler>> {
        vec![
            Box::new(FileSourceHandler::with_base_path(base_path)),
            Box::new(GitSourceHandler::new()),
            Box::new(ZipSourceHandler::new()),
            Box::new(HttpSourceHandler::new()),
        ]
    }

    /// Add a custom source handler.
    ///
    /// Custom handlers are inserted at the front and take priority
    /// over default handlers. First match wins.
    pub fn add_handler(&mut self, handler: Box<dyn SourceHandler>) {
        self.handlers.insert(0, handler);
    }

    /// Resolve a URI to local paths.
    ///
    /// Parses the URI and tries each registered handler in order.
    /// Returns the result from the first handler that can handle the URI.
    ///
    /// # Errors
    ///
    /// Returns [`BundleError::NotFound`](crate::error::BundleError::NotFound)
    /// if no handler can resolve the URI.
    pub async fn resolve(&self, uri: &str) -> crate::error::Result<ResolvedSource> {
        let parsed = parse_uri(uri);

        for handler in &self.handlers {
            if handler.can_handle(&parsed) {
                return handler.resolve(&parsed, &self.cache_dir).await;
            }
        }

        Err(crate::error::BundleError::NotFound {
            uri: uri.to_string(),
        })
    }
}

/// `SimpleSourceResolver` implements the `SourceResolver` protocol trait.
///
/// This bridges the concrete implementation with the abstract protocol,
/// allowing `SimpleSourceResolver` to be used wherever a `dyn SourceResolver`
/// is accepted.
#[async_trait]
impl SourceResolver for SimpleSourceResolver {
    async fn resolve(&self, uri: &str) -> crate::error::Result<ResolvedSource> {
        // Delegate to the inherent method
        SimpleSourceResolver::resolve(self, uri).await
    }
}
