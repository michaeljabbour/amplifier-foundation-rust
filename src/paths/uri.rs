use std::path::{Path, PathBuf};

pub fn get_amplifier_home() -> PathBuf {
    todo!()
}

#[derive(Debug, Clone)]
pub struct ParsedURI {
    pub scheme: String,
    pub host: String,
    pub path: String,
    pub ref_: String,
    pub subpath: String,
}

impl ParsedURI {
    pub fn is_git(&self) -> bool {
        todo!()
    }

    pub fn is_file(&self) -> bool {
        todo!()
    }

    pub fn is_http(&self) -> bool {
        todo!()
    }

    pub fn is_zip(&self) -> bool {
        todo!()
    }

    pub fn is_package(&self) -> bool {
        todo!()
    }
}

pub fn parse_uri(_uri: &str) -> ParsedURI {
    todo!()
}

pub fn normalize_path(_path: &str, _relative_to: Option<&Path>) -> PathBuf {
    todo!()
}

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
