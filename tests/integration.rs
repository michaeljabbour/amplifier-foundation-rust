//! Integration tests for amplifier-foundation-rs.
//!
//! These tests exercise cross-module flows with real YAML data loaded from
//! fixture files, validating the end-to-end pipeline that production code uses.

use std::path::PathBuf;

use amplifier_foundation::{
    deep_merge, validate_bundle, validate_bundle_completeness, Bundle, BundleValidator,
    CacheProvider, DiskCache, SimpleCache,
};
use serde_yaml_ng::Value;

/// Helper to get the fixtures directory path.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Helper to load and parse a YAML fixture file.
fn load_yaml_fixture(name: &str) -> Value {
    let path = fixtures_dir().join(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", name, e));
    serde_yaml_ng::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse fixture {}: {}", name, e))
}

// ============================================================================
// Test 1: Load real YAML file and parse into Bundle
// ============================================================================

#[test]
fn test_load_full_yaml_bundle() {
    let data = load_yaml_fixture("full-bundle.yaml");
    let bundle = Bundle::from_dict(&data).expect("Should parse full bundle");

    assert_eq!(bundle.name, "test-full-bundle");
    assert_eq!(bundle.version, "2.1.0");
    assert_eq!(
        bundle.description,
        "A complete bundle for integration testing"
    );

    // Providers
    assert_eq!(bundle.providers.len(), 2, "Should have 2 providers");
    let p0 = bundle.providers[0].as_mapping().unwrap();
    assert_eq!(
        p0.get("module").unwrap().as_str().unwrap(),
        "provider-anthropic"
    );

    // Tools
    assert_eq!(bundle.tools.len(), 3, "Should have 3 tools");

    // Hooks
    assert_eq!(bundle.hooks.len(), 1, "Should have 1 hook");

    // Session
    assert!(bundle.session.is_mapping(), "Session should be a mapping");
    let session = bundle.session.as_mapping().unwrap();
    assert!(session.get("orchestrator").is_some());
    assert!(session.get("context").is_some());
    assert_eq!(session.get("debug").unwrap().as_bool(), Some(true));

    // Spawn
    assert!(bundle.spawn.is_mapping(), "Spawn should be a mapping");

    // Agents
    assert_eq!(bundle.agents.len(), 2, "Should have 2 agents");
    assert!(bundle.agents.contains_key("explorer"));
    assert!(bundle.agents.contains_key("builder"));
}

#[test]
fn test_load_minimal_yaml_bundle() {
    let data = load_yaml_fixture("minimal.yaml");
    let bundle = Bundle::from_dict(&data).expect("Should parse minimal bundle");

    assert_eq!(bundle.name, "test-minimal");
    assert_eq!(bundle.version, "1.0.0");
    assert!(bundle.providers.is_empty());
    assert!(bundle.tools.is_empty());
    assert!(bundle.hooks.is_empty());
}

// ============================================================================
// Test 2: Load markdown bundle with frontmatter
// ============================================================================

#[test]
fn test_load_markdown_bundle_with_frontmatter() {
    let path = fixtures_dir().join("bundle.md");
    let content = std::fs::read_to_string(&path).expect("Should read bundle.md");

    // Parse frontmatter
    let (frontmatter, body) =
        amplifier_foundation::parse_frontmatter(&content).expect("Should parse frontmatter");

    assert!(frontmatter.is_some(), "Should have frontmatter");

    // Build bundle from frontmatter.
    // The registry wraps frontmatter in {"bundle": fm} before calling from_dict.
    // Since our .md frontmatter does NOT have a "bundle:" key (matching registry convention),
    // the wrapping produces {"bundle": {"name": ..., "providers": [...], ...}} which from_dict handles.
    let fm = frontmatter.unwrap();
    let mut wrapper = serde_yaml_ng::Mapping::new();
    wrapper.insert(Value::String("bundle".to_string()), fm);
    let mut bundle =
        Bundle::from_dict(&Value::Mapping(wrapper)).expect("Should parse bundle from frontmatter");

    // Set instruction from body (as registry does)
    let trimmed = body.trim();
    if !trimmed.is_empty() {
        bundle.instruction = Some(trimmed.to_string());
    }

    assert_eq!(bundle.name, "markdown-bundle");
    assert_eq!(bundle.version, "1.0.0");
    assert_eq!(bundle.providers.len(), 1);
    assert_eq!(bundle.tools.len(), 2);

    // Instruction should contain the markdown body
    assert!(bundle.instruction.is_some());
    let instruction = bundle.instruction.unwrap();
    assert!(
        instruction.contains("# Agent Instructions"),
        "Instruction should contain heading"
    );
    assert!(
        instruction.contains("Write clean, well-documented code"),
        "Instruction should contain guideline text"
    );
}

// ============================================================================
// Test 3: Cross-module pipeline: YAML → from_dict → compose → to_mount_plan → validate
// ============================================================================

#[test]
fn test_full_pipeline_compose_and_validate() {
    let base_data = load_yaml_fixture("full-bundle.yaml");
    let child_data = load_yaml_fixture("child-bundle.yaml");

    let base = Bundle::from_dict(&base_data).expect("Should parse base bundle");
    let child = Bundle::from_dict(&child_data).expect("Should parse child bundle");

    // Compose: child on top of base
    let composed = base.compose(&[&child]);

    // Child name wins
    assert_eq!(composed.name, "test-child-bundle");
    assert_eq!(composed.version, "3.0.0");

    // Providers: merge by module ID
    // base: provider-anthropic, provider-openai
    // child: provider-anthropic (merged), provider-google (added)
    // Result: provider-anthropic (merged), provider-openai, provider-google
    assert_eq!(
        composed.providers.len(),
        3,
        "Should have exactly 3 providers after merge"
    );

    // Check that provider-anthropic has child's config (max_tokens: 16384)
    let anthropic_provider = composed
        .providers
        .iter()
        .find(|p| {
            p.as_mapping()
                .and_then(|m| m.get("module"))
                .and_then(|v| v.as_str())
                == Some("provider-anthropic")
        })
        .expect("Should have provider-anthropic");
    let anthropic_config = anthropic_provider
        .as_mapping()
        .unwrap()
        .get("config")
        .unwrap()
        .as_mapping()
        .unwrap();
    assert_eq!(
        anthropic_config
            .get("max_tokens")
            .unwrap()
            .as_u64()
            .unwrap(),
        16384,
        "Child's max_tokens should override base"
    );

    // Tools: merge by module ID
    // base: tool-filesystem, tool-bash, tool-web
    // child: tool-filesystem (merged), tool-search (added)
    // Result: tool-filesystem (merged), tool-bash, tool-web, tool-search
    assert_eq!(
        composed.tools.len(),
        4,
        "Should have exactly 4 tools after merge"
    );

    // Session: deep merge (child's orchestrator config overrides base)
    let session = composed.session.as_mapping().expect("Session should exist");
    let orch = session.get("orchestrator").unwrap().as_mapping().unwrap();
    let orch_config = orch.get("config").unwrap().as_mapping().unwrap();
    // Child sets extended_thinking: false
    assert!(
        !orch_config
            .get("extended_thinking")
            .unwrap()
            .as_bool()
            .unwrap(),
        "Child's extended_thinking should override base to false"
    );
    // Child adds stream: true
    assert!(
        orch_config.get("stream").unwrap().as_bool().unwrap(),
        "Child's stream config should be present"
    );

    // Generate mount plan
    let mount_plan = composed.to_mount_plan();
    assert!(mount_plan.is_mapping(), "Mount plan should be a mapping");
    let plan_map = mount_plan.as_mapping().unwrap();
    assert!(
        plan_map.get("session").is_some(),
        "Plan should have session"
    );
    assert!(
        plan_map.get("providers").is_some(),
        "Plan should have providers"
    );
    assert!(plan_map.get("tools").is_some(), "Plan should have tools");

    // Validate basic structure
    let result = validate_bundle(&composed);
    assert!(
        result.valid,
        "Composed bundle should be valid: {:?}",
        result
    );

    // Validate completeness (should pass since composed has session + providers)
    let completeness = validate_bundle_completeness(&composed);
    assert!(
        completeness.valid,
        "Composed bundle should be complete: {:?}",
        completeness
    );
}

// ============================================================================
// Test 4: Mount plan is serializable to YAML and back
// ============================================================================

#[test]
fn test_mount_plan_yaml_roundtrip() {
    let data = load_yaml_fixture("full-bundle.yaml");
    let bundle = Bundle::from_dict(&data).expect("Should parse bundle");
    let mount_plan = bundle.to_mount_plan();

    // Serialize to YAML string
    let yaml_str =
        serde_yaml_ng::to_string(&mount_plan).expect("Mount plan should serialize to YAML");

    // Deserialize back
    let parsed: Value =
        serde_yaml_ng::from_str(&yaml_str).expect("Should parse mount plan YAML back");

    // Compare
    assert_eq!(
        mount_plan, parsed,
        "Mount plan should survive YAML round-trip"
    );
}

// ============================================================================
// Test 5: DiskCache with real mount plan data
// ============================================================================

#[test]
fn test_disk_cache_with_real_mount_plan() {
    let data = load_yaml_fixture("full-bundle.yaml");
    let bundle = Bundle::from_dict(&data).expect("Should parse bundle");
    let mount_plan = bundle.to_mount_plan();

    let tmp = tempfile::tempdir().expect("Should create temp dir");

    // Store mount plan in disk cache
    let mut cache = DiskCache::new(tmp.path());
    let cache_key = format!("bundle::{}", bundle.name);

    // Convert mount plan to serde_yaml_ng::Value (cache uses YAML values)
    cache.set(&cache_key, mount_plan.clone());

    // Verify it's cached
    assert!(cache.contains(&cache_key), "Cache should contain the key");

    // Retrieve and compare
    let retrieved = cache.get(&cache_key).expect("Should retrieve from cache");
    assert_eq!(
        retrieved, mount_plan,
        "Retrieved mount plan should match original"
    );

    // Simulate new process: fresh DiskCache pointing to same directory
    let cache2 = DiskCache::new(tmp.path());
    let retrieved2 = cache2
        .get(&cache_key)
        .expect("Should retrieve from fresh cache instance");
    assert_eq!(
        retrieved2, mount_plan,
        "Fresh cache instance should retrieve same data"
    );
}

// ============================================================================
// Test 6: SimpleCache with real bundle data
// ============================================================================

#[test]
fn test_simple_cache_with_bundle_data() {
    let data = load_yaml_fixture("full-bundle.yaml");
    let bundle = Bundle::from_dict(&data).expect("Should parse bundle");
    let mount_plan = bundle.to_mount_plan();

    let mut cache = SimpleCache::new();
    cache.set("test-key", mount_plan.clone());

    assert!(cache.contains("test-key"));
    assert_eq!(cache.get("test-key").unwrap(), mount_plan);

    // Clear and verify
    cache.clear();
    assert!(!cache.contains("test-key"));
}

// ============================================================================
// Test 7: Validator with real bundle data
// ============================================================================

#[test]
fn test_validator_with_real_full_bundle() {
    let data = load_yaml_fixture("full-bundle.yaml");
    let bundle = Bundle::from_dict(&data).expect("Should parse bundle");

    let validator = BundleValidator::new();

    // Basic validation
    let result = validator.validate(&bundle);
    assert!(
        result.valid,
        "Full bundle should pass basic validation: errors={:?}",
        result.errors
    );

    // Completeness validation
    let completeness = validator.validate_completeness(&bundle);
    assert!(
        completeness.valid,
        "Full bundle should pass completeness: errors={:?}",
        completeness.errors
    );
}

#[test]
fn test_validator_minimal_bundle_incomplete() {
    let data = load_yaml_fixture("minimal.yaml");
    let bundle = Bundle::from_dict(&data).expect("Should parse minimal bundle");

    let validator = BundleValidator::new();

    // Basic validation should pass (name is present)
    let result = validator.validate(&bundle);
    assert!(result.valid, "Minimal bundle should pass basic validation");

    // Completeness should fail (no session, no providers)
    let completeness = validator.validate_completeness(&bundle);
    assert!(
        !completeness.valid,
        "Minimal bundle should fail completeness check"
    );
    assert!(
        !completeness.errors.is_empty(),
        "Should have completeness errors"
    );
}

// ============================================================================
// Test 8: deep_merge with real session configs
// ============================================================================

#[test]
fn test_deep_merge_real_session_configs() {
    let base_data = load_yaml_fixture("full-bundle.yaml");
    let child_data = load_yaml_fixture("child-bundle.yaml");

    let base = Bundle::from_dict(&base_data).unwrap();
    let child = Bundle::from_dict(&child_data).unwrap();

    // Deep merge the sessions (strategy 1 from compose)
    let merged = deep_merge(&base.session, &child.session);

    let merged_map = merged.as_mapping().expect("Merged should be mapping");

    // Orchestrator should be deeply merged
    let orch = merged_map
        .get("orchestrator")
        .unwrap()
        .as_mapping()
        .unwrap();
    let orch_config = orch.get("config").unwrap().as_mapping().unwrap();

    // Child's extended_thinking=false overwrites base's true
    assert_eq!(
        orch_config.get("extended_thinking").unwrap().as_bool(),
        Some(false)
    );

    // Child adds stream=true
    assert_eq!(orch_config.get("stream").unwrap().as_bool(), Some(true));

    // Base's max_tokens from orchestrator config should survive if child doesn't set it
    // (base has max_tokens: 200000 in orchestrator config but child has no max_tokens in orchestrator)
    assert_eq!(
        orch_config.get("max_tokens").unwrap().as_u64(),
        Some(200000)
    );

    // Base's context should survive (child has no context)
    let ctx = merged_map.get("context").unwrap().as_mapping().unwrap();
    assert_eq!(ctx.get("module").unwrap().as_str(), Some("context-simple"));

    // Base's debug should survive (child has no debug)
    assert_eq!(merged_map.get("debug").unwrap().as_bool(), Some(true));
}

// ============================================================================
// Test 9: Registry-style YAML loading (wrapping in {"bundle": raw})
// ============================================================================

#[test]
fn test_registry_style_yaml_loading() {
    // This simulates what BundleRegistry::load_yaml_bundle does:
    // 1. Read the YAML file (in registry format: fields at top level, no "bundle:" wrapper)
    // 2. Wrap raw YAML in {"bundle": raw}
    // 3. Call from_dict_with_base_path
    //
    // Registry-format YAML files do NOT have a "bundle:" wrapper key.
    // Fields like name, version, session, providers, tools are at the top level.
    // The registry wrapping produces {"bundle": {"name": ..., "session": ..., ...}}.

    let path = fixtures_dir().join("registry-format.yaml");
    let content = std::fs::read_to_string(&path).expect("Should read fixture");
    let raw: Value = serde_yaml_ng::from_str(&content).expect("Should parse YAML");

    let mut wrapper = serde_yaml_ng::Mapping::new();
    wrapper.insert(Value::String("bundle".to_string()), raw);

    let base_path = path.parent().unwrap();
    let bundle = Bundle::from_dict_with_base_path(&Value::Mapping(wrapper), base_path)
        .expect("Should load bundle via registry-style wrapping");

    // Name should be found
    assert_eq!(bundle.name, "registry-test-bundle");
    assert_eq!(bundle.version, "1.0.0");

    // Session should be found
    assert!(
        bundle.session.is_mapping(),
        "Session should be found in registry-style load"
    );

    // Providers and tools should be found
    assert_eq!(bundle.providers.len(), 1, "Should have 1 provider");
    assert_eq!(bundle.tools.len(), 2, "Should have 2 tools");

    // Base path should be set
    assert!(bundle.base_path.is_some(), "Base path should be set");
    assert_eq!(
        bundle.base_path.unwrap(),
        base_path,
        "Base path should match"
    );
}

// ============================================================================
// Test 10: End-to-end with parse_frontmatter → Bundle → mount_plan
// ============================================================================

#[test]
fn test_frontmatter_to_mount_plan_pipeline() {
    let path = fixtures_dir().join("bundle.md");
    let content = std::fs::read_to_string(&path).expect("Should read bundle.md");

    // Step 1: Parse frontmatter
    let (frontmatter, body) =
        amplifier_foundation::parse_frontmatter(&content).expect("Should parse");
    let fm = frontmatter.expect("Should have frontmatter");

    // Step 2: Create bundle (registry-style wrapping for .md files)
    // Frontmatter is in registry format (no "bundle:" key), so wrap it.
    let mut wrapper = serde_yaml_ng::Mapping::new();
    wrapper.insert(Value::String("bundle".to_string()), fm);
    let mut bundle = Bundle::from_dict(&Value::Mapping(wrapper)).expect("Should create bundle");
    bundle.instruction = Some(body.trim().to_string());

    // Step 3: Generate mount plan
    let mount_plan = bundle.to_mount_plan();
    let plan_map = mount_plan.as_mapping().expect("Should be mapping");

    // Mount plan should have session, providers, tools
    assert!(plan_map.get("session").is_some());
    assert!(plan_map.get("providers").is_some());
    assert!(plan_map.get("tools").is_some());

    // Mount plan should NOT have context or instruction (those go via system prompt factory)
    // (to_mount_plan excludes context and instruction by design)

    // Step 4: Validate
    let result = validate_bundle(&bundle);
    assert!(result.valid, "Bundle from .md should be valid");
}

// ============================================================================
// Test 11: Multiple composes (3-way merge)
// ============================================================================

#[test]
fn test_compose_with_multiple_overlays() {
    // Test compose(&[&a, &b]) applies overlays in order: a first, then b on top
    let base_data = load_yaml_fixture("full-bundle.yaml");
    let child_data = load_yaml_fixture("child-bundle.yaml");

    let base = Bundle::from_dict(&base_data).unwrap();
    let child = Bundle::from_dict(&child_data).unwrap();

    // Single compose call with child as overlay
    let composed = base.compose(&[&child]);

    // Verify base's tools that child doesn't touch survive
    let has_tool_bash = composed.tools.iter().any(|t| {
        t.as_mapping()
            .and_then(|m| m.get("module"))
            .and_then(|v| v.as_str())
            == Some("tool-bash")
    });
    assert!(has_tool_bash, "Base's tool-bash should survive composition");

    let has_tool_web = composed.tools.iter().any(|t| {
        t.as_mapping()
            .and_then(|m| m.get("module"))
            .and_then(|v| v.as_str())
            == Some("tool-web")
    });
    assert!(has_tool_web, "Base's tool-web should survive composition");

    // Child's new tool should appear
    let has_tool_search = composed.tools.iter().any(|t| {
        t.as_mapping()
            .and_then(|m| m.get("module"))
            .and_then(|v| v.as_str())
            == Some("tool-search")
    });
    assert!(
        has_tool_search,
        "Child's tool-search should be added in composition"
    );

    // Validation
    let result = validate_bundle(&composed);
    assert!(result.valid);

    let completeness = validate_bundle_completeness(&composed);
    assert!(completeness.valid, "Composed bundle should be complete");
}

// ============================================================================
// Test 12: Compose sequence replacement (deep_merge replaces arrays)
// ============================================================================

#[test]
fn test_compose_sequence_replacement() {
    // When composing, deep_merge replaces sequences entirely (child wins).
    // This is critical behavior: child's allowed_paths replaces base's, NOT accumulates.
    let base_data = load_yaml_fixture("full-bundle.yaml");
    let child_data = load_yaml_fixture("child-bundle.yaml");

    let base = Bundle::from_dict(&base_data).unwrap();
    let child = Bundle::from_dict(&child_data).unwrap();
    let composed = base.compose(&[&child]);

    // Find tool-filesystem in composed result
    let fs_tool = composed
        .tools
        .iter()
        .find(|t| {
            t.as_mapping()
                .and_then(|m| m.get("module"))
                .and_then(|v| v.as_str())
                == Some("tool-filesystem")
        })
        .expect("Should have tool-filesystem");

    let config = fs_tool
        .as_mapping()
        .unwrap()
        .get("config")
        .unwrap()
        .as_mapping()
        .unwrap();
    let allowed_paths = config.get("allowed_paths").unwrap().as_sequence().unwrap();

    // Child's allowed_paths: ["/workspace"] REPLACES base's ["/home/user/projects", "/tmp"]
    assert_eq!(
        allowed_paths.len(),
        1,
        "Child's allowed_paths should replace base (not accumulate)"
    );
    assert_eq!(allowed_paths[0].as_str().unwrap(), "/workspace");
}

// ============================================================================
// Test 13: Compose is non-commutative (order matters)
// ============================================================================

#[test]
fn test_compose_non_commutative() {
    let base_data = load_yaml_fixture("full-bundle.yaml");
    let child_data = load_yaml_fixture("child-bundle.yaml");

    let base = Bundle::from_dict(&base_data).unwrap();
    let child = Bundle::from_dict(&child_data).unwrap();

    let forward = base.compose(&[&child]); // child on top of base
    let reverse = child.compose(&[&base]); // base on top of child

    // Names differ (last wins)
    assert_eq!(forward.name, "test-child-bundle");
    assert_eq!(reverse.name, "test-full-bundle");

    // Versions differ
    assert_eq!(forward.version, "3.0.0");
    assert_eq!(reverse.version, "2.1.0");
}

// ============================================================================
// Test 14: to_dict produces from_dict-compatible structure (roundtrip)
// ============================================================================

#[test]
fn test_to_dict_from_dict_roundtrip() {
    // to_dict() should produce output that from_dict() can consume.
    // All fields are nested under the "bundle" key to match from_dict expectations.

    // Use inline YAML with ALL roundtrippable fields populated
    let yaml = r#"
bundle:
  name: roundtrip-test
  version: "3.0.0"
  description: "Full roundtrip test"
  session:
    orchestrator:
      module: loop-streaming
      config:
        max_tokens: 200000
    debug: true
  providers:
    - module: provider-anthropic
      config:
        model: claude-sonnet-4-20250514
    - module: provider-openai
      config:
        model: gpt-4o
  tools:
    - module: tool-filesystem
    - module: tool-bash
  hooks:
    - module: hook-shell
  agents:
    explorer:
      description: "Exploration agent"
    builder:
      description: "Build agent"
  spawn:
    default_provider: anthropic
  context:
    readme: readme.md
    guide: guide.md
  includes:
    - "./base-bundle.yaml"
    - "./extra-bundle.yaml"
"#;
    let data: Value = serde_yaml_ng::from_str(yaml).unwrap();
    let original = Bundle::from_dict(&data).unwrap();
    let dict = original.to_dict();

    // Verify structure: everything should be under "bundle" key
    let dict_map = dict.as_mapping().unwrap();
    let bundle_meta = dict_map.get("bundle").unwrap().as_mapping().unwrap();

    // All fields should be nested inside "bundle"
    for key in &[
        "name",
        "version",
        "description",
        "providers",
        "tools",
        "hooks",
        "session",
        "spawn",
        "agents",
        "context",
        "includes",
    ] {
        assert!(
            bundle_meta.get(*key).is_some(),
            "{} should be inside bundle key",
            key
        );
    }

    // Now roundtrip: from_dict(bundle.to_dict()) should produce equivalent bundle
    let roundtripped = Bundle::from_dict(&dict).unwrap();

    // Metadata
    assert_eq!(roundtripped.name, original.name);
    assert_eq!(roundtripped.version, original.version);
    assert_eq!(roundtripped.description, original.description);

    // Module lists -- compare content, not just length
    assert_eq!(roundtripped.providers, original.providers);
    assert_eq!(roundtripped.tools, original.tools);
    assert_eq!(roundtripped.hooks, original.hooks);

    // Session and spawn (Value equality)
    assert_eq!(roundtripped.session, original.session);
    assert_eq!(roundtripped.spawn, original.spawn);

    // Agents (same keys and values)
    assert_eq!(roundtripped.agents.len(), original.agents.len());
    for (name, agent) in &original.agents {
        assert_eq!(roundtripped.agents.get(name), Some(agent));
    }

    // Context (keys survive, values are path strings)
    assert_eq!(roundtripped.context.len(), original.context.len());
    let orig_keys: Vec<&String> = original.context.keys().collect();
    let rt_keys: Vec<&String> = roundtripped.context.keys().collect();
    assert_eq!(rt_keys, orig_keys, "context keys should roundtrip");

    // Includes
    assert_eq!(roundtripped.includes, original.includes);
}

#[test]
fn test_to_dict_roundtrip_minimal() {
    // Even a minimal bundle should roundtrip correctly
    let bundle = Bundle::new("minimal");
    let dict = bundle.to_dict();
    let roundtripped = Bundle::from_dict(&dict).unwrap();

    assert_eq!(roundtripped.name, "minimal");
    assert_eq!(roundtripped.version, "1.0.0");
    assert!(roundtripped.providers.is_empty());
    assert!(roundtripped.tools.is_empty());
    assert!(roundtripped.hooks.is_empty());
    assert!(roundtripped.agents.is_empty());
}

// ============================================================================
// Test 15: Bundle.agents preserves insertion order (IndexMap)
// ============================================================================

#[test]
fn test_agents_preserve_insertion_order() {
    // Parse a bundle with multiple agents -- the order in YAML should be preserved
    let yaml = r#"
bundle:
  name: ordering-test
  agents:
    alpha:
      description: "First agent"
    beta:
      description: "Second agent"
    gamma:
      description: "Third agent"
    delta:
      description: "Fourth agent"
    epsilon:
      description: "Fifth agent"
"#;
    let data: Value = serde_yaml_ng::from_str(yaml).unwrap();
    let bundle = Bundle::from_dict(&data).unwrap();

    // Agents should be in YAML insertion order
    let agent_names: Vec<&String> = bundle.agents.keys().collect();
    assert_eq!(
        agent_names,
        vec!["alpha", "beta", "gamma", "delta", "epsilon"],
        "agents should preserve insertion order from YAML"
    );

    // Mount plan should also have deterministic agent order
    let plan = bundle.to_mount_plan();
    let plan_map = plan.as_mapping().unwrap();
    let agents_section = plan_map.get("agents").unwrap().as_mapping().unwrap();
    let plan_agent_names: Vec<&str> = agents_section.keys().filter_map(|k| k.as_str()).collect();
    assert_eq!(
        plan_agent_names,
        vec!["alpha", "beta", "gamma", "delta", "epsilon"],
        "mount plan agents should preserve insertion order"
    );
}

// ============================================================================
// Test 15b: Agent ordering preserved through compose
// ============================================================================

#[test]
fn test_agents_order_preserved_through_compose() {
    let base_yaml = r#"
bundle:
  name: base
  agents:
    alpha:
      description: "A"
    beta:
      description: "B"
    gamma:
      description: "C"
"#;
    let overlay_yaml = r#"
bundle:
  name: overlay
  agents:
    beta:
      description: "B-updated"
    delta:
      description: "D"
"#;
    let base_data: Value = serde_yaml_ng::from_str(base_yaml).unwrap();
    let overlay_data: Value = serde_yaml_ng::from_str(overlay_yaml).unwrap();
    let base = Bundle::from_dict(&base_data).unwrap();
    let overlay = Bundle::from_dict(&overlay_data).unwrap();

    let composed = base.compose(&[&overlay]);

    // Existing keys preserve original position, new keys appended
    // Matches Python dict.update() semantics
    let agent_names: Vec<&String> = composed.agents.keys().collect();
    assert_eq!(
        agent_names,
        vec!["alpha", "beta", "gamma", "delta"],
        "compose should preserve base order for existing keys, append new"
    );

    // Verify beta was actually updated
    assert_eq!(
        composed.agents["beta"]["description"].as_str(),
        Some("B-updated")
    );
}

// ============================================================================
// Test 15c: Context ordering preserved through compose with namespace
// ============================================================================

#[test]
fn test_context_order_preserved_through_compose() {
    let base_yaml = r#"
bundle:
  name: base
  context:
    readme: readme.md
    guide: guide.md
"#;
    let overlay_yaml = r#"
bundle:
  name: overlay
  context:
    extra: extra.md
    notes: notes.md
"#;
    let base_data: Value = serde_yaml_ng::from_str(base_yaml).unwrap();
    let overlay_data: Value = serde_yaml_ng::from_str(overlay_yaml).unwrap();
    let base = Bundle::from_dict(&base_data).unwrap();
    let overlay = Bundle::from_dict(&overlay_data).unwrap();

    let composed = base.compose(&[&overlay]);

    // Base context first, then overlay context with namespace prefix
    let context_keys: Vec<&String> = composed.context.keys().collect();
    assert_eq!(
        context_keys,
        vec!["readme", "guide", "overlay:extra", "overlay:notes"],
        "compose should preserve base context order, then append namespaced overlay"
    );
}

// ============================================================================
// Test 16: Bundle.context preserves insertion order (IndexMap)
// ============================================================================

#[test]
fn test_context_preserves_insertion_order() {
    let yaml = r#"
bundle:
  name: context-order-test
  context:
    system-prompt: system.md
    guidelines: guidelines.md
    examples: examples.md
    reference: reference.md
"#;
    let data: Value = serde_yaml_ng::from_str(yaml).unwrap();
    let bundle = Bundle::from_dict(&data).unwrap();

    // Context entries should be in YAML insertion order
    let context_keys: Vec<&String> = bundle.context.keys().collect();
    assert_eq!(
        context_keys,
        vec!["system-prompt", "guidelines", "examples", "reference"],
        "context should preserve insertion order from YAML"
    );
}
