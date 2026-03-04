"""
Python-side smoke tests for amplifier_foundation PyO3 bindings.

Run with: maturin develop --strip && pytest tests/python/ -v

These tests validate that PyO3 bindings are correctly wired and return
the expected Python types. They complement the Rust-side unit tests
which test behavior exhaustively.
"""

import inspect
import os
import tempfile

import pytest

import amplifier_foundation as af


# =============================================================================
# Module attributes
# =============================================================================


def test_version_exists():
    assert hasattr(af, "__version__")
    assert isinstance(af.__version__, str)
    assert af.__version__  # non-empty


# =============================================================================
# Exceptions
# =============================================================================


def test_exception_hierarchy():
    """BundleError is the base; all others derive from it."""
    assert issubclass(af.BundleNotFoundError, af.BundleError)
    assert issubclass(af.BundleLoadError, af.BundleError)
    assert issubclass(af.BundleValidationError, af.BundleError)
    assert issubclass(af.BundleDependencyError, af.BundleError)
    # All bundle errors are also Exceptions
    assert issubclass(af.BundleError, Exception)


def test_exception_can_be_raised_and_caught():
    with pytest.raises(af.BundleError):
        raise af.BundleLoadError("test error")


# =============================================================================
# ParsedURI
# =============================================================================


def test_parse_uri_basic():
    uri = af.parse_uri("https://example.com/path")
    assert isinstance(uri, af.ParsedURI)
    assert uri.scheme == "https"
    assert uri.host == "example.com"
    assert uri.path == "/path"


def test_parse_uri_git():
    uri = af.parse_uri("git+https://github.com/user/repo.git@v1.0#subdirectory=sub")
    assert uri.is_git()
    assert not uri.is_http()
    assert uri.ref_ == "v1.0"
    assert uri.subpath == "sub"


def test_parse_uri_hashable():
    u1 = af.parse_uri("https://a.com/x")
    u2 = af.parse_uri("https://a.com/x")
    assert u1 == u2
    assert hash(u1) == hash(u2)
    s = {u1, u2}
    assert len(s) == 1


def test_parse_uri_repr():
    uri = af.parse_uri("file:///tmp/bundle")
    r = repr(uri)
    assert "ParsedURI" in r


# =============================================================================
# Bundle
# =============================================================================


def test_bundle_constructor():
    b = af.Bundle("my-bundle")
    assert b.name == "my-bundle"
    assert b.version == "1.0.0"  # default
    assert b.provider_count == 0


def test_bundle_from_dict():
    data = {
        "bundle": {
            "name": "test-bundle",
            "version": "2.0.0",
            "description": "A test bundle",
            "providers": [{"module": "provider-openai", "config": {"model": "gpt-4o"}}],
        }
    }
    b = af.Bundle.from_dict(data)
    assert b.name == "test-bundle"
    assert b.version == "2.0.0"
    assert b.description == "A test bundle"
    assert b.provider_count == 1


def test_bundle_from_dict_invalid():
    with pytest.raises(af.BundleLoadError):
        af.Bundle.from_dict({"bundle": {"providers": "not-a-list"}})


def test_bundle_compose():
    base = af.Bundle.from_dict(
        {"bundle": {"name": "base", "providers": [{"module": "provider-a"}]}}
    )
    overlay = af.Bundle.from_dict(
        {"bundle": {"name": "overlay", "providers": [{"module": "provider-b"}]}}
    )
    composed = base.compose([overlay])
    assert composed.name == "overlay"  # overlay wins
    assert composed.provider_count == 2  # merged


def test_bundle_to_dict_roundtrip():
    data = {
        "bundle": {
            "name": "roundtrip",
            "version": "1.0.0",
            "providers": [{"module": "provider-x"}],
        }
    }
    b = af.Bundle.from_dict(data)
    d = b.to_dict()
    assert "bundle" in d
    assert d["bundle"]["name"] == "roundtrip"


def test_bundle_to_mount_plan():
    data = {
        "bundle": {
            "name": "mp",
            "providers": [{"module": "provider-a"}],
            "tools": [{"module": "tool-b"}],
        }
    }
    b = af.Bundle.from_dict(data)
    plan = b.to_mount_plan()
    assert "providers" in plan
    assert "tools" in plan
    assert len(plan["providers"]) == 1


def test_bundle_setters():
    b = af.Bundle("old")
    b.name = "new"
    b.version = "3.0.0"
    assert b.name == "new"
    assert b.version == "3.0.0"


def test_bundle_copy():
    import copy

    b = af.Bundle("original")
    c = copy.copy(b)
    assert c.name == "original"
    c.name = "copy"
    assert b.name == "original"  # original unchanged


def test_bundle_hash_is_identity():
    """Bundle uses default identity-based hash (not value-based)."""
    b1 = af.Bundle("same")
    b2 = af.Bundle("same")
    # Different objects have different hashes (identity, not value)
    assert hash(b1) != hash(b2)


# =============================================================================
# ValidationResult
# =============================================================================


def test_validate_bundle():
    b = af.Bundle.from_dict({"bundle": {"name": "valid", "providers": [{"module": "p"}]}})
    result = af.validate_bundle(b)
    assert isinstance(result, af.ValidationResult)
    assert result.is_valid
    assert bool(result) is True
    assert len(result.errors) == 0


def test_validate_bundle_or_raise():
    b = af.Bundle()
    b.name = ""  # invalid - missing name
    with pytest.raises(af.BundleValidationError):
        af.validate_bundle_or_raise(b)


# =============================================================================
# SourceStatus
# =============================================================================


def test_source_status():
    s = af.SourceStatus("git+https://example.com/repo")
    assert s.uri == "git+https://example.com/repo"
    assert s.has_update is None  # unknown by default
    assert bool(s) is False  # unknown is falsy


def test_source_status_hashable():
    s1 = af.SourceStatus("a")
    s2 = af.SourceStatus("a")
    assert s1 == s2
    assert hash(s1) == hash(s2)


# =============================================================================
# ResolvedSource
# =============================================================================


def test_resolved_source():
    rs = af.ResolvedSource("/path/to/sub", "/path/to")
    assert rs.active_path == "/path/to/sub"
    assert rs.source_root == "/path/to"
    assert rs.is_subdirectory()


# =============================================================================
# ProviderPreference
# =============================================================================


def test_provider_preference():
    p = af.ProviderPreference("anthropic", "claude-*")
    assert p.provider == "anthropic"
    assert p.model == "claude-*"
    d = p.to_dict()
    assert d == {"provider": "anthropic", "model": "claude-*"}


def test_provider_preference_from_dict():
    p = af.ProviderPreference.from_dict({"provider": "openai", "model": "gpt-4o"})
    assert p.provider == "openai"


def test_provider_preference_from_list():
    prefs = af.ProviderPreference.from_list([
        {"provider": "a", "model": "m1"},
        {"provider": "b", "model": "m2"},
    ])
    assert len(prefs) == 2


# =============================================================================
# SimpleCache
# =============================================================================


def test_simple_cache():
    c = af.SimpleCache()
    assert c.get("key") is None
    c.set("key", {"hello": "world"})
    assert c.get("key") == {"hello": "world"}
    assert c.contains("key")
    assert "key" in c
    c.clear()
    assert c.get("key") is None


# =============================================================================
# DiskCache
# =============================================================================


def test_disk_cache():
    with tempfile.TemporaryDirectory() as tmpdir:
        cache_dir = os.path.join(tmpdir, "cache")
        c = af.DiskCache(cache_dir)
        assert c.cache_dir == cache_dir
        c.set("k", [1, 2, 3])
        assert c.get("k") == [1, 2, 3]
        assert c.contains("k")
        path = c.cache_key_to_path("k")
        assert isinstance(path, str)
        c.clear()
        assert c.get("k") is None


# =============================================================================
# Dict utilities
# =============================================================================


def test_deep_merge():
    result = af.deep_merge({"a": 1, "b": {"c": 2}}, {"b": {"d": 3}})
    assert result == {"a": 1, "b": {"c": 2, "d": 3}}


def test_deep_merge_type_error():
    with pytest.raises(TypeError):
        af.deep_merge("not a dict", {})


def test_get_nested():
    data = {"a": {"b": {"c": 42}}}
    assert af.get_nested(data, ["a", "b", "c"]) == 42
    assert af.get_nested(data, ["a", "x"]) is None


def test_set_nested():
    data = {"a": {"b": 1}}
    result = af.set_nested(data, ["a", "c"], 2)
    assert result["a"]["c"] == 2
    assert data["a"].get("c") is None  # original not mutated


# =============================================================================
# Serialization
# =============================================================================


def test_sanitize_for_json():
    result = af.sanitize_for_json({"a": 1, "b": None, "c": [None, 2]})
    assert result == {"a": 1, "c": [2]}


def test_sanitize_message():
    msg = {"role": "assistant", "content": "hello"}
    result = af.sanitize_message(msg)
    assert result["role"] == "assistant"


def test_merge_module_lists():
    parent = [{"module": "a", "x": 1}]
    child = [{"module": "a", "y": 2}, {"module": "b"}]
    result = af.merge_module_lists(parent, child)
    assert len(result) == 2
    # 'a' should be merged
    a_entry = next(e for e in result if e["module"] == "a")
    assert a_entry.get("x") == 1
    assert a_entry.get("y") == 2


# =============================================================================
# Mentions and tracing
# =============================================================================


def test_parse_mentions():
    mentions = af.parse_mentions("Use @file.py and @dir/other.txt in context")
    assert "@file.py" in mentions or "file.py" in mentions


def test_generate_sub_session_id():
    sid = af.generate_sub_session_id(agent_name="test-agent")
    assert isinstance(sid, str)
    assert len(sid) > 0


# =============================================================================
# IO
# =============================================================================


def test_parse_frontmatter():
    content = "---\ntitle: Hello\n---\nBody text"
    fm, body = af.parse_frontmatter(content)
    assert fm is not None
    assert fm.get("title") == "Hello"
    assert "Body text" in body


def test_parse_frontmatter_no_frontmatter():
    fm, body = af.parse_frontmatter("Just plain text")
    assert fm is None
    assert body == "Just plain text"


# =============================================================================
# Session
# =============================================================================


def test_count_turns():
    messages = [
        {"role": "user", "content": "Hello"},
        {"role": "assistant", "content": "Hi"},
        {"role": "user", "content": "How?"},
    ]
    assert af.count_turns(messages) == 2


def test_get_turn_boundaries():
    messages = [
        {"role": "user", "content": "A"},
        {"role": "assistant", "content": "B"},
        {"role": "user", "content": "C"},
    ]
    boundaries = af.get_turn_boundaries(messages)
    assert boundaries == [0, 2]


def test_fork_session_in_memory():
    messages = [
        {"role": "user", "content": "Hello"},
        {"role": "assistant", "content": "Hi there"},
    ]
    result = af.fork_session_in_memory(messages)
    assert isinstance(result, af.ForkResult)
    assert result.session_id  # non-empty
    assert result.message_count == 2
    assert result.messages is not None
    assert len(result.messages) == 2


def test_fork_result_hash_is_identity():
    """ForkResult uses default identity-based hash (not value-based)."""
    messages = [{"role": "user", "content": "x"}, {"role": "assistant", "content": "y"}]
    r1 = af.fork_session_in_memory(messages)
    r2 = af.fork_session_in_memory(messages)
    # Different objects have different hashes (each has unique session_id)
    assert hash(r1) != hash(r2)


# =============================================================================
# Paths
# =============================================================================


def test_normalize_path():
    result = af.normalize_path("/tmp/../tmp/test")
    assert result == "/tmp/test"


def test_get_amplifier_home():
    home = af.get_amplifier_home()
    assert isinstance(home, str)
    assert home.endswith(".amplifier")


def test_is_glob_pattern():
    assert af.is_glob_pattern("claude-*")
    assert not af.is_glob_pattern("gpt-4o")


# =============================================================================
# text_signature verification
# =============================================================================


def test_text_signatures_present():
    """Verify inspect.signature works on key functions."""
    sig = inspect.signature(af.parse_uri)
    assert "uri" in str(sig)

    sig = inspect.signature(af.deep_merge)
    assert "base" in str(sig)
    assert "overlay" in str(sig)

    sig = inspect.signature(af.generate_sub_session_id)
    params = list(sig.parameters.keys())
    assert "agent_name" in params
    assert "session_id" in params
    assert "trace_id" in params