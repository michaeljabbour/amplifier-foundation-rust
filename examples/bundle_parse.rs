//! Example: Parsing and inspecting a bundle from YAML data.
//!
//! Demonstrates:
//! - Creating a Bundle from YAML data via `Bundle::from_dict`
//! - Inspecting bundle fields (name, version, providers, tools)
//! - Generating a mount plan with `to_mount_plan`
//! - Validating a bundle with `BundleValidator`
//!
//! Run with: cargo run --example bundle_parse

use amplifier_foundation::{Bundle, BundleValidator, validate_bundle};

fn main() {
    println!("=== Bundle Parsing Example ===\n");

    // 1. Parse a bundle from YAML data
    let yaml = r#"
bundle:
  name: my-assistant
  version: "1.0.0"
  description: "An example AI assistant bundle"
  session:
    orchestrator: standard
    context:
      key: session-context-value
  providers:
    - module: provider-anthropic
      config:
        model: claude-sonnet-4-20250514
    - module: provider-openai
      config:
        model: gpt-4
  tools:
    - module: tool-filesystem
      config:
        allowed_paths: ["/tmp"]
  hooks:
    - module: hook-logging
"#;

    let data: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
    let bundle = Bundle::from_dict(&data).unwrap();

    println!("Bundle: {} v{}", bundle.name, bundle.version);
    println!("Description: {}", bundle.description);
    println!("Providers: {} configured", bundle.providers.len());
    println!("Tools: {} configured", bundle.tools.len());
    println!("Hooks: {} configured", bundle.hooks.len());

    // 2. Generate a mount plan
    let mount_plan = bundle.to_mount_plan();
    println!("\n--- Mount Plan ---");
    let plan_yaml = serde_yaml_ng::to_string(&mount_plan).unwrap();
    println!("{}", plan_yaml);

    // 3. Validate the bundle
    let result = validate_bundle(&bundle);
    println!("--- Validation ---");
    println!("Valid: {}", result.valid);
    if !result.errors.is_empty() {
        println!("Errors: {:?}", result.errors);
    }
    if !result.warnings.is_empty() {
        println!("Warnings: {:?}", result.warnings);
    }

    // 4. Completeness check (stricter)
    let validator = BundleValidator::new();
    let completeness = validator.validate_completeness(&bundle);
    println!("\n--- Completeness Check ---");
    println!("Complete: {}", completeness.valid);
    if !completeness.errors.is_empty() {
        for err in &completeness.errors {
            println!("  - {}", err);
        }
    }
}