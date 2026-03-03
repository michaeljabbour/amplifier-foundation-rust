//! Registry data types: UpdateInfo and BundleState.

/// Information about an available update for a registered bundle.
///
/// Returned by registry update-checking operations. Matches Python's
/// `UpdateInfo` dataclass in `registry.py`.
///
/// This is the **bundle-level** update notification, produced when the registry
/// determines that a newer version is available. It is distinct from
/// [`SourceStatus`](crate::sources::SourceStatus) which is a **source-level**
/// status check (may be unknown/tri-state). `UpdateInfo` represents a *confirmed*
/// update — `available_version` is always known (non-optional).
///
/// Currently a data-only struct. Will be returned by `BundleRegistry` update-checking
/// methods when full update workflow is implemented.
///
/// # Examples
///
/// ```
/// use amplifier_foundation::UpdateInfo;
///
/// let info = UpdateInfo {
///     name: "my-bundle".to_string(),
///     current_version: Some("1.0.0".to_string()),
///     available_version: "2.0.0".to_string(),
///     uri: "git+https://github.com/org/my-bundle@main".to_string(),
/// };
/// assert_eq!(info.name, "my-bundle");
/// assert_eq!(info.current_version.as_deref(), Some("1.0.0"));
/// assert_eq!(info.available_version, "2.0.0");
/// assert!(info.uri.starts_with("git+"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct UpdateInfo {
    /// Name of the bundle with an update available.
    pub name: String,
    /// Currently installed version, if known.
    pub current_version: Option<String>,
    /// Version available for update (always known for confirmed updates).
    pub available_version: String,
    /// Source URI of the bundle.
    pub uri: String,
}

/// Tracked state for a registered bundle.
///
/// Terminology:
///   Root bundle: A bundle at /bundle.md or /bundle.yaml at the root of a repo
///       or directory tree. Establishes the namespace and root directory for
///       path resolution. Tracked via is_root=True.
///
///   Nested bundle: A bundle loaded via #subdirectory= URIs or @namespace:path
///       references. Shares the namespace with its root bundle and resolves
///       paths relative to its own location. Tracked via is_root=False.
#[derive(Debug, Clone)]
pub struct BundleState {
    pub uri: String,
    pub name: String,
    pub version: Option<String>,
    /// When this bundle was last loaded (ISO 8601 string).
    /// Stored as String to avoid forcing a chrono dependency on consumers.
    pub loaded_at: Option<String>,
    /// When this bundle was last checked for updates (ISO 8601 string).
    pub checked_at: Option<String>,
    pub local_path: Option<String>,
    pub includes: Vec<String>,
    pub included_by: Vec<String>,
    pub is_root: bool,
    pub root_name: Option<String>,
    pub explicitly_requested: bool,
    pub app_bundle: bool,
}

impl BundleState {
    pub fn new(name: &str, uri: &str) -> Self {
        BundleState {
            uri: uri.to_string(),
            name: name.to_string(),
            version: None,
            loaded_at: None,
            checked_at: None,
            local_path: None,
            includes: Vec::new(),
            included_by: Vec::new(),
            is_root: true,
            root_name: None,
            explicitly_requested: false,
            app_bundle: false,
        }
    }

    pub fn to_dict(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "uri".to_string(),
            serde_json::Value::String(self.uri.clone()),
        );
        map.insert(
            "name".to_string(),
            serde_json::Value::String(self.name.clone()),
        );
        if let Some(v) = &self.version {
            map.insert("version".to_string(), serde_json::Value::String(v.clone()));
        }
        if let Some(la) = &self.loaded_at {
            map.insert(
                "loaded_at".to_string(),
                serde_json::Value::String(la.clone()),
            );
        }
        if let Some(ca) = &self.checked_at {
            map.insert(
                "checked_at".to_string(),
                serde_json::Value::String(ca.clone()),
            );
        }
        if let Some(lp) = &self.local_path {
            map.insert(
                "local_path".to_string(),
                serde_json::Value::String(lp.clone()),
            );
        }
        map.insert("is_root".to_string(), serde_json::Value::Bool(self.is_root));
        map.insert(
            "explicitly_requested".to_string(),
            serde_json::Value::Bool(self.explicitly_requested),
        );
        map.insert(
            "app_bundle".to_string(),
            serde_json::Value::Bool(self.app_bundle),
        );
        if !self.includes.is_empty() {
            map.insert(
                "includes".to_string(),
                serde_json::Value::Array(
                    self.includes
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        if !self.included_by.is_empty() {
            map.insert(
                "included_by".to_string(),
                serde_json::Value::Array(
                    self.included_by
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        if let Some(rn) = &self.root_name {
            map.insert(
                "root_name".to_string(),
                serde_json::Value::String(rn.clone()),
            );
        }
        serde_json::Value::Object(map)
    }

    pub fn from_dict(name: &str, data: &serde_json::Value) -> Self {
        let obj = data.as_object();
        BundleState {
            uri: obj
                .and_then(|o| o.get("uri"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            name: name.to_string(),
            version: obj
                .and_then(|o| o.get("version"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            loaded_at: obj
                .and_then(|o| o.get("loaded_at"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            checked_at: obj
                .and_then(|o| o.get("checked_at"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            local_path: obj
                .and_then(|o| o.get("local_path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            includes: obj
                .and_then(|o| o.get("includes"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            included_by: obj
                .and_then(|o| o.get("included_by"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            is_root: obj
                .and_then(|o| o.get("is_root"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            root_name: obj
                .and_then(|o| o.get("root_name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            explicitly_requested: obj
                .and_then(|o| o.get("explicitly_requested"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            app_bundle: obj
                .and_then(|o| o.get("app_bundle"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }
}
