use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use amplifier_foundation::mentions::dedup::ContentDeduplicator;
use amplifier_foundation::mentions::loader::format_context_block;
use amplifier_foundation::mentions::parser::parse_mentions;
use amplifier_foundation::mentions::resolver::BaseMentionResolver;
use serial_test::serial;

// ---------------------------------------------------------------------------
// TestParseMentions
// ---------------------------------------------------------------------------

#[test]

fn test_no_mentions() {
    let result = parse_mentions("Hello world");
    assert!(result.is_empty());
}

#[test]

fn test_simple_mention() {
    let result = parse_mentions("Check @file.md for details");
    assert_eq!(result, vec!["@file.md"]);
}

#[test]

fn test_multiple_mentions() {
    let result = parse_mentions("See @first.md and @second.md");
    let result_set: HashSet<String> = result.into_iter().collect();
    let expected: HashSet<String> = ["@first.md", "@second.md"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(result_set, expected);
}

#[test]

fn test_namespaced_mention() {
    let result = parse_mentions("Follow @foundation:philosophy");
    assert_eq!(result, vec!["@foundation:philosophy"]);
}

#[test]

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

fn test_mention_in_inline_code_excluded() {
    let text = "Use `@code.md` but also @real.md";
    let result = parse_mentions(text);
    assert!(result.contains(&"@real.md".to_string()));
    assert!(!result.contains(&"@code.md".to_string()));
}

#[test]

fn test_mention_with_path() {
    let result = parse_mentions("Check @docs/guide.md");
    assert_eq!(result, vec!["@docs/guide.md"]);
}

#[test]

fn test_deduplication() {
    let result = parse_mentions("@file.md and @file.md again");
    assert_eq!(result, vec!["@file.md"]);
}

#[test]

fn test_tilde_home_path() {
    let result = parse_mentions("Check @~/.amplifier/AGENTS.md");
    assert!(result.contains(&"@~/.amplifier/AGENTS.md".to_string()));
}

#[test]

fn test_dot_directory_path() {
    let result = parse_mentions("Check @.amplifier/AGENTS.md");
    assert!(result.contains(&"@.amplifier/AGENTS.md".to_string()));
}

#[test]

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
#[serial]
fn test_resolve_simple_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("AGENTS.md");
    fs::write(&file_path, "agent content").unwrap();

    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();

    let resolver = BaseMentionResolver::new();
    let resolved = resolver.resolve("@AGENTS.md");
    assert!(resolved.is_some());
    assert_eq!(
        resolved.unwrap().canonicalize().unwrap(),
        file_path.canonicalize().unwrap()
    );

    std::env::set_current_dir(old_dir).unwrap();
}

#[test]
#[serial]
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
#[serial]
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
#[serial]
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
#[serial]
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
#[serial]
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
#[serial]
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
#[serial]
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

fn test_resolve_bundle_pattern_unchanged() {
    let resolver = BaseMentionResolver::with_bundles(HashMap::new());
    let resolved = resolver.resolve("@foundation:context/file.md");
    assert!(resolved.is_none());
}

#[test]
#[serial]
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

// ---------------------------------------------------------------------------
// TestLoadMentions
// ---------------------------------------------------------------------------

use amplifier_foundation::mentions::loader::load_mentions;

#[tokio::test]
async fn test_load_mentions_basic_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("notes.md");
    fs::write(&file_path, "Some content here").unwrap();

    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    let result = load_mentions("Check @notes.md for details", &resolver).await;

    assert_eq!(result.files.len(), 1);
    assert_eq!(result.files[0].mention, "@notes.md");
    assert_eq!(result.files[0].content, "Some content here");
    assert!(result.failed.is_empty());
}

#[tokio::test]
async fn test_load_mentions_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    let result = load_mentions("Check @nonexistent.md", &resolver).await;

    // Missing files are silently skipped (opportunistic)
    assert!(result.files.is_empty());
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0], "@nonexistent.md");
}

#[tokio::test]
async fn test_load_mentions_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let dir_path = tmp.path().join("mydir");
    fs::create_dir_all(&dir_path).unwrap();
    fs::write(dir_path.join("file.txt"), "hello").unwrap();

    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    let result = load_mentions("See @mydir", &resolver).await;

    assert_eq!(result.files.len(), 1);
    assert_eq!(result.files[0].mention, "@mydir");
    // Directory listing should contain the file
    assert!(result.files[0].content.contains("file.txt"));
    assert!(result.files[0].content.contains("Directory:"));
    assert!(result.failed.is_empty());
}

#[tokio::test]
async fn test_load_mentions_deduplication() {
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("README.md");
    fs::write(&file_path, "Same content").unwrap();

    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    // Same mention twice in text -- parse_mentions already deduplicates
    let result = load_mentions("See @README.md and again @README.md", &resolver).await;

    assert_eq!(result.files.len(), 1);
}

#[tokio::test]
async fn test_load_mentions_multiple_files() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a.md"), "Content A").unwrap();
    fs::write(tmp.path().join("b.md"), "Content B").unwrap();

    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    let result = load_mentions("Check @a.md and @b.md", &resolver).await;

    assert_eq!(result.files.len(), 2);
    assert!(result.failed.is_empty());

    let mentions: Vec<&str> = result.files.iter().map(|f| f.mention.as_str()).collect();
    assert!(mentions.contains(&"@a.md"));
    assert!(mentions.contains(&"@b.md"));
}

#[tokio::test]
async fn test_load_mentions_recursive() {
    let tmp = tempfile::tempdir().unwrap();
    // parent.md mentions child.md
    fs::write(tmp.path().join("parent.md"), "See @child.md for more").unwrap();
    fs::write(tmp.path().join("child.md"), "Child content").unwrap();

    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    let result = load_mentions("Start with @parent.md", &resolver).await;

    // Should load both parent.md and child.md (recursive)
    assert_eq!(result.files.len(), 2);
    let mentions: Vec<&str> = result.files.iter().map(|f| f.mention.as_str()).collect();
    assert!(mentions.contains(&"@parent.md"));
    assert!(mentions.contains(&"@child.md"));
}

#[tokio::test]
async fn test_load_mentions_max_depth() {
    let tmp = tempfile::tempdir().unwrap();
    // Chain: a -> b -> c -> d (depth 3 should stop at c)
    fs::write(tmp.path().join("a.md"), "See @b.md").unwrap();
    fs::write(tmp.path().join("b.md"), "See @c.md").unwrap();
    fs::write(tmp.path().join("c.md"), "See @d.md").unwrap();
    fs::write(tmp.path().join("d.md"), "End").unwrap();

    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    // Default max_depth=3: a(0)->b(1)->c(2)->d(3), d is at depth 3 which is still resolved
    let result = load_mentions("Start @a.md", &resolver).await;

    let mentions: Vec<&str> = result.files.iter().map(|f| f.mention.as_str()).collect();
    assert!(mentions.contains(&"@a.md"));
    assert!(mentions.contains(&"@b.md"));
    assert!(mentions.contains(&"@c.md"));
    assert!(mentions.contains(&"@d.md"));
}

#[tokio::test]
async fn test_load_mentions_no_mentions() {
    let tmp = tempfile::tempdir().unwrap();
    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    let result = load_mentions("No mentions here", &resolver).await;

    assert!(result.files.is_empty());
    assert!(result.failed.is_empty());
}

#[tokio::test]
async fn test_load_mentions_duplicate_content_different_files() {
    let tmp = tempfile::tempdir().unwrap();
    // Two files with identical content
    fs::write(tmp.path().join("copy1.md"), "Identical content").unwrap();
    fs::write(tmp.path().join("copy2.md"), "Identical content").unwrap();

    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    let result = load_mentions("See @copy1.md and @copy2.md", &resolver).await;

    // First file loaded, second is deduplicated (same content)
    assert_eq!(result.files.len(), 1);
    assert_eq!(result.files[0].mention, "@copy1.md");
}

#[tokio::test]
async fn test_load_mentions_circular_references() {
    let tmp = tempfile::tempdir().unwrap();
    // a.md mentions b.md, b.md mentions a.md (circular)
    fs::write(tmp.path().join("a.md"), "See @b.md").unwrap();
    fs::write(tmp.path().join("b.md"), "See @a.md").unwrap();

    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    let result = load_mentions("Start @a.md", &resolver).await;

    // Both files should be loaded, dedup breaks the cycle
    assert_eq!(result.files.len(), 2);
    let mentions: Vec<&str> = result.files.iter().map(|f| f.mention.as_str()).collect();
    assert!(mentions.contains(&"@a.md"));
    assert!(mentions.contains(&"@b.md"));
}

#[tokio::test]
async fn test_load_mentions_parent_before_children() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("parent.md"), "See @child.md for more").unwrap();
    fs::write(tmp.path().join("child.md"), "Child content").unwrap();

    let resolver = BaseMentionResolver::with_base_path(tmp.path().to_path_buf());
    let result = load_mentions("Start with @parent.md", &resolver).await;

    // Parent should appear BEFORE child in the result (encounter order)
    assert_eq!(result.files.len(), 2);
    assert_eq!(result.files[0].mention, "@parent.md");
    assert_eq!(result.files[1].mention, "@child.md");
}

// ---------------------------------------------------------------------------
// format_context_block
// ---------------------------------------------------------------------------

#[test]
fn test_format_context_block_empty_deduplicator() {
    let dedup = ContentDeduplicator::new();
    let result = format_context_block(&dedup, None);
    assert_eq!(result, "");
}

#[test]
fn test_format_context_block_single_file() {
    let mut dedup = ContentDeduplicator::new();
    let path = PathBuf::from("/home/user/project/README.md");
    dedup.add_file(&path, "Hello, world!");

    let result = format_context_block(&dedup, None);
    // Verify XML structure: opening tag, content, closing tag
    assert!(result.starts_with("<context_file paths=\""));
    assert!(result.ends_with("</context_file>"));
    assert!(result.contains("Hello, world!"));
    assert!(result.contains("README.md"));
    // Verify exactly one block
    assert_eq!(result.matches("<context_file").count(), 1);
    assert_eq!(result.matches("</context_file>").count(), 1);
}

#[test]
fn test_format_context_block_with_mention_to_path() {
    let mut dedup = ContentDeduplicator::new();
    let path = PathBuf::from("/home/user/project/AGENTS.md");
    dedup.add_file(&path, "Agent instructions here");

    let mut mention_to_path = HashMap::new();
    mention_to_path.insert("@AGENTS.md".to_string(), path.clone());

    let result = format_context_block(&dedup, Some(&mention_to_path));
    // Should show @AGENTS.md → <path> format in the paths attribute
    assert!(result.contains("@AGENTS.md →"));
    assert!(result.contains("AGENTS.md"));
    // Content must be inside the XML block (between tags)
    let content_start = result.find(">\n").unwrap() + 2;
    let content_end = result.find("\n</context_file>").unwrap();
    assert_eq!(
        &result[content_start..content_end],
        "Agent instructions here"
    );
}

#[test]
fn test_format_context_block_multiple_files() {
    let mut dedup = ContentDeduplicator::new();
    dedup.add_file(&PathBuf::from("/path/a.md"), "Content A");
    dedup.add_file(&PathBuf::from("/path/b.md"), "Content B");

    let result = format_context_block(&dedup, None);
    // Should have two context_file blocks
    assert_eq!(result.matches("<context_file").count(), 2);
    assert_eq!(result.matches("</context_file>").count(), 2);
    assert!(result.contains("Content A"));
    assert!(result.contains("Content B"));
    // Blocks should be separated by double newline
    assert!(result.contains("</context_file>\n\n<context_file"));
}

#[test]
fn test_format_context_block_duplicate_content_different_paths() {
    let mut dedup = ContentDeduplicator::new();
    dedup.add_file(&PathBuf::from("/path/a.md"), "Same content");
    dedup.add_file(&PathBuf::from("/path/b.md"), "Same content");

    let result = format_context_block(&dedup, None);
    // Should produce ONE block (deduplicated) but show both paths
    assert_eq!(result.matches("<context_file").count(), 1);
    assert!(result.contains("Same content"));
    // Both paths should appear in the paths attribute
    assert!(result.contains("a.md"));
    assert!(result.contains("b.md"));
}

#[test]
fn test_format_context_block_mention_with_namespace() {
    let mut dedup = ContentDeduplicator::new();
    let path = PathBuf::from("/bundles/foundation/context/KERNEL.md");
    dedup.add_file(&path, "Kernel documentation");

    let mut mention_to_path = HashMap::new();
    mention_to_path.insert("@foundation:context/KERNEL.md".to_string(), path.clone());

    let result = format_context_block(&dedup, Some(&mention_to_path));
    assert!(result.contains("@foundation:context/KERNEL.md →"));
    assert!(result.contains("Kernel documentation"));
    // Content should be between tags
    assert!(result.starts_with("<context_file paths=\""));
    assert!(result.ends_with("</context_file>"));
}

#[test]
fn test_format_context_block_with_real_files() {
    // Test with real files on disk to exercise fs::canonicalize success path
    let tmp = tempfile::tempdir().unwrap();
    let file_path = tmp.path().join("real_file.md");
    fs::write(&file_path, "Real file content").unwrap();

    let mut dedup = ContentDeduplicator::new();
    dedup.add_file(&file_path, "Real file content");

    let mut mention_to_path = HashMap::new();
    mention_to_path.insert("@real_file.md".to_string(), file_path.clone());

    let result = format_context_block(&dedup, Some(&mention_to_path));
    // With real files, canonicalize succeeds on both sides, so mention attribution works
    assert!(result.contains("@real_file.md →"));
    assert!(result.contains("Real file content"));
    assert_eq!(result.matches("<context_file").count(), 1);
    // The resolved path should be absolute (from canonicalize)
    let canonical = fs::canonicalize(&file_path).unwrap();
    assert!(result.contains(&canonical.display().to_string()));
}
