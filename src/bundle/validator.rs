use serde_yaml_ng::Value;

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
        BundleValidator
    }

    /// Basic validation: checks required fields and module list format.
    /// Permissive -- missing session is OK for partial bundles.
    pub fn validate(&self, bundle: &Bundle) -> ValidationResult {
        let mut result = ValidationResult::new();

        // 1. Required fields
        self.validate_required_fields(bundle, &mut result);

        // 2. Module lists (providers, tools, hooks)
        self.validate_module_lists(bundle, &mut result);

        result
    }

    /// Validate and raise on failure.
    pub fn validate_or_raise(&self, bundle: &Bundle) -> crate::error::Result<()> {
        let result = self.validate(bundle);
        if !result.valid {
            return Err(crate::error::BundleError::LoadError {
                reason: format!(
                    "Bundle validation failed: {}",
                    result.errors.join("; ")
                ),
                source: None,
            });
        }
        Ok(())
    }

    /// Completeness validation: stricter check for mountable bundles.
    /// Runs basic validate() first, then checks for session, orchestrator, context, providers.
    pub fn validate_completeness(&self, bundle: &Bundle) -> ValidationResult {
        let mut result = self.validate(bundle);

        // Session must exist and be non-null
        if bundle.session.is_null() {
            result.add_error("Bundle is missing required 'session' section");
        } else if let Some(session_map) = bundle.session.as_mapping() {
            // Orchestrator must be present
            let orch_key = Value::String("orchestrator".to_string());
            if session_map.get(&orch_key).is_none() {
                result.add_error("Bundle session is missing required 'orchestrator'");
            }

            // Context must be present
            let ctx_key = Value::String("context".to_string());
            if session_map.get(&ctx_key).is_none() {
                result.add_error("Bundle session is missing required 'context'");
            }
        } else {
            // Session exists but is not a mapping
            result.add_error("Bundle 'session' must be a mapping");
        }

        // At least one provider required
        if bundle.providers.is_empty() {
            result.add_error("Bundle must have at least one provider");
        }

        result
    }

    /// Validate completeness and raise on failure.
    pub fn validate_completeness_or_raise(
        &self,
        bundle: &Bundle,
    ) -> crate::error::Result<()> {
        let result = self.validate_completeness(bundle);
        if !result.valid {
            return Err(crate::error::BundleError::LoadError {
                reason: format!(
                    "Bundle incomplete for mounting: {}",
                    result.errors.join("; ")
                ),
                source: None,
            });
        }
        Ok(())
    }

    // -- Private validation helpers --

    /// Check that required fields are present.
    fn validate_required_fields(&self, bundle: &Bundle, result: &mut ValidationResult) {
        if bundle.name.is_empty() {
            result.add_error("Bundle must have a name");
        }
    }

    /// Check that all module list entries are valid.
    fn validate_module_lists(&self, bundle: &Bundle, result: &mut ValidationResult) {
        self.validate_module_list_entries("providers", &bundle.providers, result);
        self.validate_module_list_entries("tools", &bundle.tools, result);
        self.validate_module_list_entries("hooks", &bundle.hooks, result);
    }

    /// Validate individual entries in a module list.
    fn validate_module_list_entries(
        &self,
        list_name: &str,
        entries: &[Value],
        result: &mut ValidationResult,
    ) {
        for (i, entry) in entries.iter().enumerate() {
            self.validate_module_entry(list_name, i, entry, result);
        }
    }

    /// Validate a single module entry.
    fn validate_module_entry(
        &self,
        list_name: &str,
        index: usize,
        entry: &Value,
        result: &mut ValidationResult,
    ) {
        // Must be a mapping
        let map = match entry.as_mapping() {
            Some(m) => m,
            None => {
                let type_name = value_type_name(entry);
                result.add_error(&format!(
                    "{}[{}]: Must be a dict, got {}",
                    list_name, index, type_name
                ));
                return;
            }
        };

        // Must have "module" field
        let module_key = Value::String("module".to_string());
        if map.get(&module_key).is_none() {
            result.add_error(&format!(
                "{}[{}]: Missing required 'module' field",
                list_name, index
            ));
        }

        // "config" must be a mapping if present
        let config_key = Value::String("config".to_string());
        if let Some(config) = map.get(&config_key) {
            if !config.is_mapping() {
                let type_name = value_type_name(config);
                result.add_error(&format!(
                    "{}[{}]: 'config' must be a dict, got {}",
                    list_name, index, type_name
                ));
            }
        }
    }
}

/// Convenience function: validate a bundle.
pub fn validate_bundle(bundle: &Bundle) -> ValidationResult {
    BundleValidator::new().validate(bundle)
}

/// Convenience function: validate or raise.
pub fn validate_bundle_or_raise(bundle: &Bundle) -> crate::error::Result<()> {
    BundleValidator::new().validate_or_raise(bundle)
}

/// Convenience function: validate completeness.
pub fn validate_bundle_completeness(bundle: &Bundle) -> ValidationResult {
    BundleValidator::new().validate_completeness(bundle)
}

/// Convenience function: validate completeness or raise.
pub fn validate_bundle_completeness_or_raise(bundle: &Bundle) -> crate::error::Result<()> {
    BundleValidator::new().validate_completeness_or_raise(bundle)
}

/// Helper: get human-readable type name for a YAML Value.
fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "str",
        Value::Sequence(_) => "list",
        Value::Mapping(_) => "dict",
        Value::Tagged(_) => "tagged",
    }
}
