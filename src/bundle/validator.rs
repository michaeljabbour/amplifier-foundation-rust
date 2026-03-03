use crate::bundle::Bundle;

/// Validation result with add_error/add_warning methods.
/// Note: This ValidationResult is richer than the one in error.rs.
/// The error.rs ValidationResult is used for the BundleError::ValidationError variant.
/// This one is the working validator result used during validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationResult {
    pub fn new() -> Self {
        ValidationResult {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_error(&mut self, message: &str) {
        self.errors.push(message.to_string());
        self.valid = false;
    }

    pub fn add_warning(&mut self, message: &str) {
        self.warnings.push(message.to_string());
    }
}

pub struct BundleValidator;

impl Default for BundleValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl BundleValidator {
    pub fn new() -> Self {
        todo!()
    }

    pub fn validate(&self, _bundle: &Bundle) -> ValidationResult {
        todo!()
    }

    pub fn validate_or_raise(&self, _bundle: &Bundle) -> crate::error::Result<()> {
        todo!()
    }

    pub fn validate_completeness(&self, _bundle: &Bundle) -> ValidationResult {
        todo!()
    }

    pub fn validate_completeness_or_raise(&self, _bundle: &Bundle) -> crate::error::Result<()> {
        todo!()
    }
}

/// Convenience function: validate a bundle.
pub fn validate_bundle(_bundle: &Bundle) -> ValidationResult {
    todo!()
}

/// Convenience function: validate or raise.
pub fn validate_bundle_or_raise(_bundle: &Bundle) -> crate::error::Result<()> {
    todo!()
}

/// Convenience function: validate completeness.
pub fn validate_bundle_completeness(_bundle: &Bundle) -> ValidationResult {
    todo!()
}

/// Convenience function: validate completeness or raise.
pub fn validate_bundle_completeness_or_raise(_bundle: &Bundle) -> crate::error::Result<()> {
    todo!()
}
