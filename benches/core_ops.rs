//! Criterion benchmarks for amplifier-foundation core operations.
//!
//! Run with: `cargo bench`
//! Results in: `target/criterion/`

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_yaml_ng::Value;

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn make_bundle_yaml() -> Value {
    serde_yaml_ng::from_str(
        r#"
bundle:
  name: bench-bundle
  version: "2.0"
  providers:
    - module: provider-openai
      config:
        model: gpt-4
        temperature: 0.7
    - module: provider-anthropic
      config:
        model: claude-3-opus
  tools:
    - module: tool-bash
      config:
        timeout: 30
    - module: tool-read-file
    - module: tool-write-file
  hooks:
    - module: hook-safety
      config:
        level: strict
  session:
    orchestrator:
      module: orchestrator-default
      config:
        max_turns: 50
    context:
      module: context-default
  spawn:
    default_model: gpt-4
  agents:
    coder:
      instruction: "You are a coder."
      model: gpt-4
    reviewer:
      instruction: "You are a reviewer."
      model: claude-3-opus
  context:
    overview: context/overview.md
    guidelines: context/guidelines.md
"#,
    )
    .unwrap()
}

fn make_child_bundle_yaml() -> Value {
    serde_yaml_ng::from_str(
        r#"
bundle:
  name: child-bundle
  version: "1.0"
  providers:
    - module: provider-anthropic
      config:
        model: claude-3-sonnet
  tools:
    - module: tool-web-search
  session:
    orchestrator:
      config:
        max_turns: 100
"#,
    )
    .unwrap()
}

fn make_session_mapping() -> Value {
    serde_yaml_ng::from_str(
        r#"
orchestrator:
  module: orchestrator-custom
  config:
    max_turns: 200
    retry_count: 3
context:
  module: context-default
  config:
    depth: 5
capabilities:
  working_dir: /workspace
  allowed_paths:
    - /workspace
    - /tmp
"#,
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// Benchmarks: deep_merge
// ---------------------------------------------------------------------------

fn bench_deep_merge(c: &mut Criterion) {
    let base = make_session_mapping();
    let overlay = serde_yaml_ng::from_str(
        r#"
orchestrator:
  config:
    max_turns: 500
    new_field: added
context:
  config:
    depth: 10
capabilities:
  working_dir: /new-workspace
"#,
    )
    .unwrap();

    c.bench_function("deep_merge/session_configs", |b| {
        b.iter(|| amplifier_foundation::deep_merge(black_box(&base), black_box(&overlay)))
    });
}

// ---------------------------------------------------------------------------
// Benchmarks: from_dict
// ---------------------------------------------------------------------------

fn bench_from_dict(c: &mut Criterion) {
    let data = make_bundle_yaml();

    c.bench_function("from_dict/full_bundle", |b| {
        b.iter(|| amplifier_foundation::Bundle::from_dict(black_box(&data)).unwrap())
    });

    let minimal: Value = serde_yaml_ng::from_str(
        r#"
bundle:
  name: minimal
  version: "1.0"
"#,
    )
    .unwrap();

    c.bench_function("from_dict/minimal_bundle", |b| {
        b.iter(|| amplifier_foundation::Bundle::from_dict(black_box(&minimal)).unwrap())
    });
}

// ---------------------------------------------------------------------------
// Benchmarks: compose
// ---------------------------------------------------------------------------

fn bench_compose(c: &mut Criterion) {
    let parent_data = make_bundle_yaml();
    let child_data = make_child_bundle_yaml();
    let parent = amplifier_foundation::Bundle::from_dict(&parent_data).unwrap();
    let child = amplifier_foundation::Bundle::from_dict(&child_data).unwrap();

    c.bench_function("compose/two_bundles", |b| {
        b.iter(|| black_box(&parent).compose(&[black_box(&child)]))
    });
}

// ---------------------------------------------------------------------------
// Benchmarks: parse_mentions
// ---------------------------------------------------------------------------

fn bench_parse_mentions(c: &mut Criterion) {
    let text = "Here is some text with @mention1 and @path/to/file.md \
                and @./relative/path and some email user@example.com \
                ```\n@code_block_mention\n``` and `@inline_code` \
                and @another/mention and @bundle:name/path.yaml";

    c.bench_function("parse_mentions/mixed_text", |b| {
        b.iter(|| amplifier_foundation::parse_mentions(black_box(text)))
    });
}

criterion_group!(
    benches,
    bench_deep_merge,
    bench_from_dict,
    bench_compose,
    bench_parse_mentions
);
criterion_main!(benches);
