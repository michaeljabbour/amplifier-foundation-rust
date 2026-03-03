use std::path::{Component, Path, PathBuf};

/// Get the Amplifier home directory.
///
/// Resolves in order:
/// 1. AMPLIFIER_HOME environment variable (expanded, resolved)
/// 2. ~/.amplifier (default)
pub fn get_amplifier_home() -> PathBuf {
    if let Ok(env_home) = std::env::var("AMPLIFIER_HOME") {
        let expanded = expand_tilde(&env_home);
        let p = PathBuf::from(expanded);
        return std::fs::canonicalize(&p).unwrap_or_else(|_| make_absolute(&p));
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let p = home.join(".amplifier");
    std::fs::canonicalize(&p).unwrap_or_else(|_| make_absolute(&p))
}

/// Expand ~ in path strings to home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), stripped);
        }
    }
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// Make a path absolute without filesystem access.
fn make_absolute(p: &Path) -> PathBuf {
    if p.is_absolute() {
        normalize_components(p)
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        normalize_components(&cwd.join(p))
    }
}

/// Normalize path components by resolving . and .. without filesystem access.
fn normalize_components(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {} // skip .
            Component::ParentDir => {
                // Only pop Normal components, not root/prefix
                if matches!(components.last(), Some(Component::Normal(_))) {
                    components.pop();
                }
            }
            c => components.push(c),
        }
    }
    components.iter().collect()
}

/// Parsed URI components.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedURI {
    pub scheme: String,
    pub host: String,
    pub path: String,
    pub ref_: String,
    pub subpath: String,
}

impl ParsedURI {
    /// True if this is a git URI.
    pub fn is_git(&self) -> bool {
        self.scheme == "git" || self.scheme.starts_with("git+")
    }

    /// True if this is a file URI or local path.
    pub fn is_file(&self) -> bool {
        self.scheme == "file" || (self.scheme.is_empty() && self.path.contains('/'))
    }

    /// True if this is an HTTP/HTTPS URI.
    pub fn is_http(&self) -> bool {
        self.scheme == "http" || self.scheme == "https"
    }

    /// True if this is a zip URI (zip+https://, zip+file://).
    pub fn is_zip(&self) -> bool {
        self.scheme.starts_with("zip+")
    }

    /// True if this looks like a package/bundle name.
    pub fn is_package(&self) -> bool {
        self.scheme.is_empty() && !self.path.contains('/')
    }
}

/// Parse a URI string into components.
///
/// Handles: git+https://..., file://..., /absolute/path, ./relative/path,
/// https://..., zip+https://..., package-name, name@ref
pub fn parse_uri(uri: &str) -> ParsedURI {
    // Handle git+ prefix (pip/uv standard)
    if uri.starts_with("git+") {
        return parse_vcs_uri(uri, "git+");
    }

    // Handle zip+ prefix (extended pattern for archives)
    if uri.starts_with("zip+") {
        return parse_vcs_uri(uri, "zip+");
    }

    // Handle explicit file:// scheme
    if let Some(remainder) = uri.strip_prefix("file://") {
        let (path, subpath) = extract_fragment_subpath(remainder);
        return ParsedURI {
            scheme: "file".to_string(),
            host: String::new(),
            path,
            ref_: String::new(),
            subpath,
        };
    }

    // Handle absolute paths
    if uri.starts_with('/') {
        return ParsedURI {
            scheme: "file".to_string(),
            host: String::new(),
            path: uri.to_string(),
            ref_: String::new(),
            subpath: String::new(),
        };
    }

    // Handle relative paths
    if uri.starts_with("./") || uri.starts_with("../") {
        return ParsedURI {
            scheme: "file".to_string(),
            host: String::new(),
            path: uri.to_string(),
            ref_: String::new(),
            subpath: String::new(),
        };
    }

    // Handle http/https URLs
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return parse_http_uri(uri);
    }

    // Package name with subpath (contains /)
    if uri.contains('/') {
        let (name, rest) = uri.split_once('/').unwrap();
        return ParsedURI {
            scheme: String::new(),
            host: String::new(),
            path: name.to_string(),
            ref_: String::new(),
            subpath: rest.to_string(),
        };
    }

    // Bare package name
    ParsedURI {
        scheme: String::new(),
        host: String::new(),
        path: uri.to_string(),
        ref_: String::new(),
        subpath: String::new(),
    }
}

/// Normalize a path relative to a base directory.
///
/// Handles: absolute paths, relative paths, ~/ expansion, ./prefix.
/// Does NOT touch the filesystem (no symlink resolution).
pub fn normalize_path(path: &str, relative_to: Option<&Path>) -> PathBuf {
    let p = Path::new(path);

    if p.is_absolute() {
        return normalize_components(p);
    }

    if let Some(base) = relative_to {
        return normalize_components(&base.join(p));
    }

    // Use current working directory
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    normalize_components(&cwd.join(p))
}

/// Result of resolving a source URI to local paths.
#[derive(Debug, Clone)]
pub struct ResolvedSource {
    pub active_path: PathBuf,
    pub source_root: PathBuf,
}

impl ResolvedSource {
    pub fn is_subdirectory(&self) -> bool {
        self.active_path != self.source_root
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extract subdirectory= value from URL fragment.
fn extract_subdirectory_from_fragment(fragment: &str) -> String {
    if fragment.is_empty() {
        return String::new();
    }
    for part in fragment.split('&') {
        if let Some(value) = part.strip_prefix("subdirectory=") {
            return value.to_string();
        }
    }
    String::new()
}

/// Split a URI into (path, subpath) from #subdirectory= fragment.
fn extract_fragment_subpath(uri: &str) -> (String, String) {
    if let Some((path, fragment)) = uri.split_once('#') {
        let subpath = extract_subdirectory_from_fragment(fragment);
        (path.to_string(), subpath)
    } else {
        (uri.to_string(), String::new())
    }
}

/// Parse a VCS URI (git+ or zip+ prefix).
fn parse_vcs_uri(uri: &str, prefix: &str) -> ParsedURI {
    let uri_without_prefix = &uri[prefix.len()..];

    // Extract fragment (#subdirectory=)
    let (uri_no_fragment, subpath) = extract_fragment_subpath(uri_without_prefix);

    // Parse URL components (scheme, host, path)
    let (scheme, host, path) = parse_url_components(&uri_no_fragment);

    // Extract @ref from path (e.g., /org/repo@main)
    let (final_path, ref_) = extract_ref_from_path(&path);

    ParsedURI {
        scheme: format!("{prefix}{scheme}"),
        host,
        path: final_path,
        ref_,
        subpath,
    }
}

/// Parse HTTP/HTTPS URL into components.
fn parse_http_uri(uri: &str) -> ParsedURI {
    let (uri_no_fragment, subpath) = extract_fragment_subpath(uri);
    let (scheme, host, path) = parse_url_components(&uri_no_fragment);

    ParsedURI {
        scheme,
        host,
        path,
        ref_: String::new(),
        subpath,
    }
}

/// Parse URL components: scheme, host, path.
/// Strips query strings (`?...`) from path to match Python's `urlparse` behavior.
fn parse_url_components(url: &str) -> (String, String, String) {
    if let Some(idx) = url.find("://") {
        let scheme = url[..idx].to_string();
        let rest = &url[idx + 3..];

        if let Some(slash_idx) = rest.find('/') {
            let host = rest[..slash_idx].to_string();
            let path_with_query = &rest[slash_idx..];
            // Strip query string to match Python's urlparse behavior
            let path = match path_with_query.find('?') {
                Some(q_idx) => path_with_query[..q_idx].to_string(),
                None => path_with_query.to_string(),
            };
            (scheme, host, path)
        } else {
            (scheme, rest.to_string(), String::new())
        }
    } else {
        (String::new(), String::new(), url.to_string())
    }
}

/// Extract @ref from a path component.
///
/// Matches Python's `re.match(r"^([^@]+)@([^/]+)$", path)`:
/// - Path part: everything before the FIRST @
/// - Ref part: everything after the first @, must not contain /
fn extract_ref_from_path(path: &str) -> (String, String) {
    if let Some(at_idx) = path.find('@') {
        let potential_path = &path[..at_idx];
        let potential_ref = &path[at_idx + 1..];
        // ref must be non-empty and not contain /
        if !potential_path.is_empty() && !potential_ref.is_empty() && !potential_ref.contains('/') {
            return (potential_path.to_string(), potential_ref.to_string());
        }
    }
    (path.to_string(), String::new())
}
