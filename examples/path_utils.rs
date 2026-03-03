//! Example: Path utilities for URI parsing and path construction.
//!
//! Demonstrates:
//! - Parsing various URI formats with `parse_uri`
//! - Normalizing paths with `normalize_path`
//! - Constructing agent and context paths
//! - Using `get_amplifier_home` for platform-specific home directory
//! - Deep merging YAML values
//!
//! Run with: cargo run --example path_utils

use amplifier_foundation::{
    construct_agent_path, construct_context_path, deep_merge, get_amplifier_home, normalize_path,
    parse_uri,
};
use std::path::PathBuf;

fn main() {
    println!("=== Path Utilities Example ===\n");

    // 1. Parse various URI formats
    let uris = vec![
        "file:///home/user/bundles/my-bundle",
        "https://github.com/org/bundle-repo",
        "git+https://github.com/org/repo.git#ref=main",
        "zip+file:///path/to/bundle.zip",
        "my-package-name",
    ];

    println!("--- URI Parsing ---");
    for uri in &uris {
        let parsed = parse_uri(uri);
        println!("URI: {}", uri);
        println!(
            "  scheme: {:?}",
            if parsed.scheme.is_empty() {
                "none"
            } else {
                &parsed.scheme
            }
        );
        println!(
            "  host: {:?}",
            if parsed.host.is_empty() {
                "none"
            } else {
                &parsed.host
            }
        );
        println!("  path: {:?}", &parsed.path);
        println!(
            "  is_file: {}, is_http: {}, is_git: {}, is_zip: {}, is_package: {}",
            parsed.is_file(),
            parsed.is_http(),
            parsed.is_git(),
            parsed.is_zip(),
            parsed.is_package()
        );
        println!();
    }

    // 2. Normalize paths
    println!("--- Path Normalization ---");
    let paths = vec![
        "/home/user/../user/bundles/./my-bundle",
        "relative/path/./to/../bundle",
    ];
    for path in &paths {
        let normalized = normalize_path(path, None);
        println!("{} -> {}", path, normalized.display());
    }

    // 3. Construct paths
    println!("\n--- Path Construction ---");
    let base = PathBuf::from("/home/user/.amplifier");
    let agent_path = construct_agent_path(&base, "code-reviewer");
    let context_path = construct_context_path(&base, "system-prompt.md");
    println!("Agent path: {}", agent_path.display());
    println!("Context path: {}", context_path.display());

    // 4. Amplifier home directory
    println!("\n--- Amplifier Home ---");
    println!("Home: {}", get_amplifier_home().display());

    // 5. Deep merge example
    println!("\n--- Deep Merge ---");
    let base: serde_yaml_ng::Value = serde_yaml_ng::from_str("a: 1\nb:\n  x: 10\n  y: 20").unwrap();
    let overlay: serde_yaml_ng::Value =
        serde_yaml_ng::from_str("b:\n  y: 30\n  z: 40\nc: 3").unwrap();
    let merged = deep_merge(&base, &overlay);
    println!(
        "Base:    {}",
        serde_yaml_ng::to_string(&base).unwrap().trim()
    );
    println!(
        "Overlay: {}",
        serde_yaml_ng::to_string(&overlay).unwrap().trim()
    );
    println!(
        "Merged:  {}",
        serde_yaml_ng::to_string(&merged).unwrap().trim()
    );
}
