use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;

use amplifier_foundation::mentions::parser::parse_mentions;
use amplifier_foundation::mentions::resolver::BaseMentionResolver;

// ---------------------------------------------------------------------------
// TestParseMentions
// ---------------------------------------------------------------------------

#[test]
#[ignore = "Wave 2"]
fn test_no_mentions() {
    let result = parse_mentions("Hello world");
    assert!(result.is_empty());
}

#[test]
#[ignore = "Wave 2"]
fn test_simple_mention() {
    let result = parse_mentions("Check @file.md for details");
    assert_eq!(result, vec!["@file.md"]);
}

#[test]
#[ignore = "Wave 2"]
fn test_multiple_mentions() {
    let result = parse_mentions("See @first.md and @second.md");
    let result_set: HashSet<String> = result.into_iter().collect();
    let expected: HashSet<String> =
        ["@first.md", "@second.md"].iter().map(|s| s.to_string()).collect();
    assert_eq!(result_set, expected);
}

#[test]
#[ignore = "Wave 2"]
fn test_namespaced_mention() {
    let result = parse_mentions("Follow @foundation:philosophy");
    assert_eq!(result, vec!["@foundation:philosophy"]);
}

#[test]
#[ignore = "Wave 2"]
fn test_mention_in_code_block_excluded() {
    let text = "\
@outside.md
```python
@inside.md
```
@after.md";
    let result = parse_mentions(text);
    let result_set: HashSet<String> = result.into_iter().collect();
    assert!(result_set.contains("@outside.md"));
    assert!(result_set.contains("@after.md"));
    assert!(!result_set.contains("@inside.md"));
}

#[test]
#[ignore = "Wave 2"]
fn test_mention_in_inline_code_excluded() {
    let text = "Use `@code.md` but also @real.md";
    let result = parse_mentions(text);
    assert!(result.contains(&"@real.md".to_string()));
    assert!(!result.contains(&"@code.md".to_string()));
}

#[test]
#[ignore = "Wave 2"]
fn test_mention_with_path() {
    let result = parse_mentions("Check @docs/guide.md");
    assert_eq!(result, vec!["@docs/guide.md"]);
}

#[test]
#[ignore = "Wave 2"]
fn test_deduplication() {
    let result = parse_mentions("@file.md and @file.md again");
    assert_eq!(result, vec!["@file.md"]);
}

#[test]
#[ignore = "Wave 2"]
fn test_tilde_home_path() {
    let result = parse_mentions("Check @~/.amplifier/AGENTS.md");
    assert!(result.contains(&"@~/.amplifier/AGENTS.md".to_string()));
}

#[test]
#[ignore = "Wave 2"]
fn test_dot_directory_path() {
    let result = parse_mentions("Check @.amplifier/AGENTS.md");
    assert!(result.contains(&"@.amplifier/AGENTS.md".to_string()));
}

#[test]
#[ignore = "Wave 2"]
fn test_explicit_relative_path() {
    let result = parse_mentions("See @./subdir/file.md and @./.amplifier/AGENTS.md");
    let result_set: HashSet<String> = result.into_iter().collect();
    assert!(result_set.contains("@./subdir/file.md"));
    assert!(result_set.contains("@./.amplifier/AGENTS.md"));
}

// ---------------------------------------------------------------------------
// TestBaseMentionResolver
// ---------------------------------------------------------------------------

#[test]
#[ignore = "Wave 2"]
fn test_resolve_simple_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("AGENTS.md");
    fs::write(&file_path, "agent content").unwrap();

    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();

    let resolver = BaseMentionResolver::new();
    let resolved = resolver.resolve("@AGENTS.md");
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap().canonicalize().unwrap(), file_path.canonicalize().unwrap());

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[ignore = "Wave 2"]
fn test_resolve_dot_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(".amplifier");
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("AGENTS.md");
    fs::write(&file_path, "agent content").unwrap();

    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();

    let resolver = BaseMentionResolver::new();
    let resolved = resolver.resolve("@.amplifier/AGENTS.md");
    assert!(resolved.is_some());
    assert_eq!(
        resolved.unwrap().canonicalize().unwrap(),
        file_path.canonicalize().unwrap(),
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[ignore = "Wave 2"]
fn test_resolve_explicit_relative() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("AGENTS.md");
    fs::write(&file_path, "agent content").unwrap();

    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();

    let resolver = BaseMentionResolver::new();
    let resolved = resolver.resolve("@./AGENTS.md");
    assert!(resolved.is_some());
    // Should resolve to the same canonical path as the file
    assert_eq!(
        resolved.unwrap().canonicalize().unwrap(),
        file_path.canonicalize().unwrap(),
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[ignore = "Wave 2"]
fn test_resolve_explicit_relative_subdir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(".amplifier");
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("AGENTS.md");
    fs::write(&file_path, "agent content").unwrap();

    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();

    let resolver = BaseMentionResolver::new();
    let resolved = resolver.resolve("@./.amplifier/AGENTS.md");
    assert!(resolved.is_some());
    assert_eq!(
        resolved.unwrap().canonicalize().unwrap(),
        file_path.canonicalize().unwrap(),
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[ignore = "Wave 2"]
fn test_resolve_home_tilde_path() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_home = tmp.path().join("fakehome");
    let amp_dir = fake_home.join(".amplifier");
    fs::create_dir_all(&amp_dir).unwrap();
    let file_path = amp_dir.join("AGENTS.md");
    fs::write(&file_path, "home agent content").unwrap();

    // Point HOME to our fake home directory
    unsafe { std::env::set_var("HOME", &fake_home) };

    let resolver = BaseMentionResolver::new();
    let resolved = resolver.resolve("@~/.amplifier/AGENTS.md");
    assert!(resolved.is_some());
    assert_eq!(
        resolved.unwrap().canonicalize().unwrap(),
        file_path.canonicalize().unwrap(),
    );
}

#[test]
#[ignore = "Wave 2"]
fn test_resolve_home_tilde_md_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_home = tmp.path().join("fakehome");
    let amp_dir = fake_home.join(".amplifier");
    fs::create_dir_all(&amp_dir).unwrap();
    let file_path = amp_dir.join("AGENTS.md");
    fs::write(&file_path, "home agent content").unwrap();

    // Point HOME to our fake home directory
    unsafe { std::env::set_var("HOME", &fake_home) };

    let resolver = BaseMentionResolver::new();
    // Mention without .md extension should fall back to finding AGENTS.md
    let resolved = resolver.resolve("@~/.amplifier/AGENTS");
    assert!(resolved.is_some());
    assert_eq!(
        resolved.unwrap().canonicalize().unwrap(),
        file_path.canonicalize().unwrap(),
    );
}

#[test]
#[ignore = "Wave 2"]
fn test_resolve_md_extension_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("AGENTS.md");
    fs::write(&file_path, "agent content").unwrap();

    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();

    let resolver = BaseMentionResolver::new();
    // Mention without .md extension should fall back to finding AGENTS.md
    let resolved = resolver.resolve("@AGENTS");
    assert!(resolved.is_some());
    assert_eq!(
        resolved.unwrap().canonicalize().unwrap(),
        file_path.canonicalize().unwrap(),
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[ignore = "Wave 2"]
fn test_resolve_not_found_returns_none() {
    let tmp = tempfile::tempdir().unwrap();

    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();

    let resolver = BaseMentionResolver::new();
    let resolved = resolver.resolve("@nonexistent.md");
    assert!(resolved.is_none());

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[ignore = "Wave 2"]
fn test_resolve_bundle_pattern_unchanged() {
    let resolver = BaseMentionResolver::with_bundles(HashMap::new());
    let resolved = resolver.resolve("@foundation:context/file.md");
    assert!(resolved.is_none());
}

#[test]
#[ignore = "Wave 2"]
fn test_resolve_uses_base_path_not_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let base_dir = tmp.path().join("base");
    fs::create_dir_all(&base_dir).unwrap();
    let file_path = base_dir.join("AGENTS.md");
    fs::write(&file_path, "agent in base").unwrap();

    // Also create a different file in a separate directory we'll chdir into
    let other_dir = tmp.path().join("other");
    fs::create_dir_all(&other_dir).unwrap();

    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&other_dir).unwrap();

    // Resolver with explicit base_path should resolve relative to base_dir,
    // NOT the current working directory (other_dir).
    let resolver = BaseMentionResolver::with_base_path(base_dir.clone());
    let resolved = resolver.resolve("@AGENTS.md");
    assert!(resolved.is_some());
    assert_eq!(
        resolved.unwrap().canonicalize().unwrap(),
        file_path.canonicalize().unwrap(),
    );

    // Verify that a default resolver (no base_path) would NOT find it from CWD
    let default_resolver = BaseMentionResolver::new();
    let not_found = default_resolver.resolve("@AGENTS.md");
    assert!(not_found.is_none());

    std::env::set_current_dir(old_dir).unwrap();
}
