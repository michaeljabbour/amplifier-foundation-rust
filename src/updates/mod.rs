use crate::sources::SourceStatus;

/// Status of a bundle for update checking.
#[derive(Debug, Clone)]
pub struct BundleStatus {
    pub name: String,
    pub source_status: Option<SourceStatus>,
}

pub async fn check_bundle_status(uri: &str) -> crate::error::Result<BundleStatus> {
    todo!()
}

pub async fn update_bundle(uri: &str) -> crate::error::Result<()> {
    todo!()
}
