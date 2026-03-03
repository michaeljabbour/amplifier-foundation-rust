//! Example: Composing multiple bundles together.
//!
//! Demonstrates:
//! - Creating bundles from YAML
//! - Composing bundles with the 5-strategy merge system
//! - How providers merge by module ID (not overwrite)
//! - How session config deep-merges
//! - How context accumulates with namespace prefixing
//!
//! Run with: cargo run --example bundle_compose

use amplifier_foundation::Bundle;

fn main() {
    println!("=== Bundle Composition Example ===\n");

    // 1. Create a base bundle (like a foundation bundle)
    let base_yaml = r#"
bundle:
  name: base-foundation
  version: "1.0"
  session:
    orchestrator: standard
    max_turns: 50
  providers:
    - module: provider-anthropic
      config:
        model: claude-sonnet-4-20250514
        max_tokens: 4096
  tools:
    - module: tool-filesystem
  context:
    base-file: context/overview.md
"#;

    let base_data: serde_yaml_ng::Value = serde_yaml_ng::from_str(base_yaml).unwrap();
    let base = Bundle::from_dict(&base_data).unwrap();
    println!("Base bundle: {} (providers: {}, tools: {})",
        base.name, base.providers.len(), base.tools.len());

    // 2. Create a child bundle that extends the base
    let child_yaml = r#"
bundle:
  name: code-reviewer
  version: "2.0"
  session:
    max_turns: 100
    temperature: 0.3
  providers:
    - module: provider-anthropic
      config:
        model: claude-sonnet-4-20250514
        max_tokens: 8192
    - module: provider-openai
      config:
        model: gpt-4
  tools:
    - module: tool-git
    - module: tool-code-analysis
  context:
    review-guide: context/review-guidelines.md
"#;

    let child_data: serde_yaml_ng::Value = serde_yaml_ng::from_str(child_yaml).unwrap();
    let child = Bundle::from_dict(&child_data).unwrap();
    println!("Child bundle: {} (providers: {}, tools: {})",
        child.name, child.providers.len(), child.tools.len());

    // 3. Compose: child on top of base
    let composed = base.compose(&[&child]);

    println!("\n--- After Composition ---");
    println!("Name: {} (child wins)", composed.name);
    println!("Version: {} (child wins)", composed.version);
    println!("Providers: {} (merged by module ID)", composed.providers.len());
    println!("Tools: {} (merged by module ID)", composed.tools.len());

    // Show provider details
    println!("\nProviders after merge:");
    for (i, provider) in composed.providers.iter().enumerate() {
        let module = provider.as_mapping()
            .and_then(|m| m.get("module"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!("  {}. {}", i + 1, module);
    }

    // Show tool details
    println!("\nTools after merge:");
    for (i, tool) in composed.tools.iter().enumerate() {
        let module = tool.as_mapping()
            .and_then(|m| m.get("module"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!("  {}. {}", i + 1, module);
    }
}