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

impl BundleValidator {
    pub fn new() -> Self {
        todo!()
    }

    pub fn validate(&self, bundle: &Bundle) -> ValidationResult {
        todo!()
    }

    pub fn validate_or_raise(&self, bundle: &Bundle) -> crate::error::Result<()> {
        todo!()
    }

    pub fn validate_completeness(&self, bundle: &Bundle) -> ValidationResult {
        todo!()
    }

    pub fn validate_completeness_or_raise(&self, bundle: &Bundle) -> crate::error::Result<()> {
        todo!()
    }
}

/// Convenience function: validate a bundle.
pub fn validate_bundle(bundle: &Bundle) -> ValidationResult {
    todo!()
}

/// Convenience function: validate or raise.
pub fn validate_bundle_or_raise(bundle: &Bundle) -> crate::error::Result<()> {
    todo!()
}

/// Convenience function: validate completeness.
pub fn validate_bundle_completeness(bundle: &Bundle) -> ValidationResult {
    todo!()
}

/// Convenience function: validate completeness or raise.
pub fn validate_bundle_completeness_or_raise(bundle: &Bundle) -> crate::error::Result<()> {
    todo!()
}
