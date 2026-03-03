pub mod includes;
pub mod persistence;

use crate::bundle::Bundle;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Tracked state for a registered bundle.
#[derive(Debug, Clone)]
pub struct BundleState {
    pub uri: String,
    pub name: String,
    pub version: Option<String>,
    pub local_path: Option<String>,
    pub includes: Vec<String>,
    pub included_by: Vec<String>,
    pub is_root: bool,
    pub root_name: Option<String>,
    pub explicitly_requested: bool,
    pub app_bundle: bool,
}

/// Central bundle management.
pub struct BundleRegistry {
    home: PathBuf,
    bundles: HashMap<String, BundleState>,
    cache: HashMap<String, Bundle>,
}

impl BundleRegistry {
    pub fn new(home: PathBuf) -> Self {
        todo!()
    }

    pub fn register(&mut self, bundles: &HashMap<String, String>) {
        todo!()
    }

    pub fn unregister(&mut self, name: &str) -> bool {
        todo!()
    }

    pub fn list_registered(&self) -> Vec<String> {
        todo!()
    }

    pub fn get_state(&mut self, name: &str) -> &mut BundleState {
        todo!()
    }

    pub fn save(&self) {
        todo!()
    }

    pub fn find_nearest_bundle_file(&self, start: &Path, stop: &Path) -> Option<PathBuf> {
        todo!()
    }

    pub async fn load_single(&self, uri: &str) -> crate::error::Result<Bundle> {
        todo!()
    }

    pub async fn load(&self, uri: &str) -> crate::error::Result<Bundle> {
        todo!()
    }
}

pub async fn load_bundle(uri: &str) -> crate::error::Result<Bundle> {
    todo!()
}
