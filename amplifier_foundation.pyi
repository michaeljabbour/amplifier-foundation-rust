"""
Type stubs for amplifier_foundation — Rust-powered Python bindings.

This module provides high-performance bundle composition, validation,
session management, and utility functions for the Amplifier ecosystem.

Built with PyO3 from amplifier-foundation-rs.
"""

from typing import Any, ClassVar, Literal, Optional

__version__: str

# =============================================================================
# Exceptions
# =============================================================================

class BundleError(Exception):
    """Base exception for all bundle-related errors."""
    ...

class BundleNotFoundError(BundleError):
    """Bundle could not be located at the specified source."""
    ...

class BundleLoadError(BundleError):
    """Bundle exists but could not be loaded (parse error, invalid format)."""
    ...

class BundleValidationError(BundleError):
    """Bundle loaded but validation failed (missing required fields, etc)."""
    ...

class BundleDependencyError(BundleError):
    """Bundle dependency could not be resolved (circular deps, missing deps)."""
    ...

# =============================================================================
# Types
# =============================================================================

class ParsedURI:
    """
    Parsed URI components. Immutable (frozen).

    Access ``ref`` via ``ref_`` (trailing underscore — ``ref`` is a Python keyword).
    """

    @property
    def scheme(self) -> str: ...
    @property
    def host(self) -> str: ...
    @property
    def path(self) -> str: ...
    @property
    def subpath(self) -> str: ...
    @property
    def ref_(self) -> str: ...
    def is_git(self) -> bool: ...
    def is_file(self) -> bool: ...
    def is_http(self) -> bool: ...
    def is_zip(self) -> bool: ...
    def is_package(self) -> bool: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class Bundle:
    """
    Core composable bundle unit. Mutable.

    Create from YAML/dict via ``Bundle.from_dict(data)`` or compose
    multiple bundles via ``bundle.compose([overlay1, overlay2])``.

    Not hashable. Use ``to_dict()`` for structural comparison.
    """

    def __init__(self, name: str = "") -> None: ...
    @staticmethod
    def from_dict(data: dict[str, Any]) -> "Bundle":
        """
        Parse a bundle from a Python dict.

        Expects data with a ``"bundle"`` key containing bundle fields.

        Raises:
            BundleLoadError: If the dict structure is invalid.
            BundleValidationError: If required fields are missing.
        """
        ...
    @staticmethod
    def from_dict_with_base_path(data: dict[str, Any], base_path: str) -> "Bundle":
        """
        Parse a bundle from a dict with a filesystem base path.

        Raises:
            BundleLoadError: If the dict structure is invalid.
            BundleValidationError: If required fields are missing.
        """
        ...
    @property
    def name(self) -> str: ...
    @name.setter
    def name(self, value: str) -> None: ...
    @property
    def version(self) -> str: ...
    @version.setter
    def version(self, value: str) -> None: ...
    @property
    def description(self) -> str: ...
    @property
    def instruction(self) -> Optional[str]: ...
    @property
    def source_uri(self) -> Optional[str]: ...
    @property
    def provider_count(self) -> int: ...
    @property
    def tool_count(self) -> int: ...
    @property
    def hook_count(self) -> int: ...
    def to_dict(self) -> dict[str, Any]:
        """Serialize bundle to a Python dict."""
        ...
    def compose(self, others: list["Bundle"]) -> "Bundle":
        """
        Compose this bundle with overlays using 5-strategy merge.

        Returns a new Bundle (does not mutate self or others).
        """
        ...
    def to_mount_plan(self) -> dict[str, Any]:
        """
        Generate a mount plan dict with only non-empty sections.

        Sections: session, providers, tools, hooks, spawn, agents.
        """
        ...
    def __repr__(self) -> str: ...
    def __copy__(self) -> "Bundle": ...
    def __deepcopy__(self, memo: dict[int, Any]) -> "Bundle": ...
    __hash__: ClassVar[None]  # type: ignore[assignment]

class ValidationResult:
    """
    Bundle validation result. Immutable (frozen).

    Truthy if valid: ``if result: ...`` is equivalent to ``if result.is_valid: ...``
    """

    @property
    def is_valid(self) -> bool: ...
    @property
    def errors(self) -> list[str]: ...
    @property
    def warnings(self) -> list[str]: ...
    def __repr__(self) -> str: ...
    def __bool__(self) -> bool: ...

class SourceStatus:
    """
    Point-in-time snapshot of a bundle source's update status. Immutable (frozen).

    Truthy only when an update is confirmed available:
    ``bool(status)`` is ``True`` only if ``has_update is True``.
    Both ``has_update is False`` and ``has_update is None`` (unknown) are falsy.
    """

    def __init__(self, uri: str) -> None: ...
    @property
    def uri(self) -> str: ...
    @property
    def has_update(self) -> Optional[bool]: ...
    @property
    def is_cached(self) -> bool: ...
    @property
    def cached_at(self) -> Optional[str]: ...
    @property
    def cached_ref(self) -> Optional[str]: ...
    @property
    def cached_commit(self) -> Optional[str]: ...
    @property
    def remote_ref(self) -> Optional[str]: ...
    @property
    def remote_commit(self) -> Optional[str]: ...
    @property
    def error(self) -> Optional[str]: ...
    @property
    def summary(self) -> str: ...
    @property
    def current_version(self) -> Optional[str]: ...
    @property
    def latest_version(self) -> Optional[str]: ...
    def is_pinned(self) -> bool:
        """True if cached_ref is an exact SHA or version tag (not a branch)."""
        ...
    def __bool__(self) -> bool: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class ResolvedSource:
    """
    A source resolved to local filesystem paths. Immutable (frozen).
    """

    def __init__(self, active_path: str, source_root: str) -> None: ...
    @property
    def active_path(self) -> str: ...
    @property
    def source_root(self) -> str: ...
    def is_subdirectory(self) -> bool:
        """True if active_path is a subdirectory of source_root."""
        ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class ProviderPreference:
    """
    A preferred provider + model pair. Immutable (frozen), hashable.
    """

    def __init__(self, provider: str, model: str) -> None: ...
    @property
    def provider(self) -> str: ...
    @property
    def model(self) -> str: ...
    @staticmethod
    def from_dict(data: dict[str, str]) -> "ProviderPreference":
        """
        Create from a dict with ``"provider"`` and ``"model"`` keys.

        Raises:
            ValueError: If required keys are missing.
        """
        ...
    @staticmethod
    def from_list(data: list[dict[str, str]]) -> list["ProviderPreference"]:
        """
        Parse a list of preference dicts. Silently skips invalid entries.

        Raises:
            TypeError: If argument is not a list.
        """
        ...
    def to_dict(self) -> dict[str, str]:
        """Returns ``{"provider": "...", "model": "..."}``."""
        ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class SimpleCache:
    """
    In-memory key-value cache. Values are converted via pythonize
    (only JSON-serializable Python types are supported).
    """

    def __init__(self) -> None: ...
    def get(self, key: str) -> Optional[Any]:
        """Returns the cached value, or ``None`` if not found."""
        ...
    def set(self, key: str, value: Any) -> None:
        """
        Store a value. Must be JSON-serializable.

        Raises:
            ValueError: If value is not JSON-serializable.
        """
        ...
    def contains(self, key: str) -> bool: ...
    def clear(self) -> None:
        """Remove all entries."""
        ...
    def __contains__(self, key: str) -> bool: ...
    def __repr__(self) -> str: ...

class DiskCache:
    """
    Filesystem-backed key-value cache. Keys are SHA-256 hashed.
    Values stored as JSON files. Creates cache_dir on construction.
    """

    def __init__(self, cache_dir: str) -> None: ...
    @property
    def cache_dir(self) -> str: ...
    def get(self, key: str) -> Optional[Any]:
        """Returns the cached value, or ``None`` if not found or corrupt."""
        ...
    def set(self, key: str, value: Any) -> None:
        """
        Write value to disk as JSON. Must be JSON-serializable.

        Raises:
            ValueError: If value is not JSON-serializable.
        """
        ...
    def contains(self, key: str) -> bool: ...
    def clear(self) -> None:
        """Delete all ``.json`` files in the cache directory."""
        ...
    def cache_key_to_path(self, key: str) -> str:
        """Returns the filesystem path for a cache key (for debugging)."""
        ...
    def __contains__(self, key: str) -> bool: ...
    def __repr__(self) -> str: ...

class ForkResult:
    """
    Result of a session fork operation. Immutable (frozen).

    Not hashable — each fork produces a unique ``session_id``.

    .. note::
        The ``messages`` property performs a deep copy from Rust on each access.
        Store the result in a variable if you need to access it multiple times.
    """

    @property
    def session_id(self) -> str: ...
    @property
    def session_dir(self) -> Optional[str]: ...
    @property
    def parent_id(self) -> str: ...
    @property
    def forked_from_turn(self) -> int: ...
    @property
    def message_count(self) -> int: ...
    @property
    def messages(self) -> Optional[list[dict[str, Any]]]:
        """
        Forked messages (in-memory forks only). ``None`` for disk forks.

        .. warning::
            Each access deep-copies the data from Rust. Cache the result.
        """
        ...
    @property
    def events_count(self) -> int: ...
    def __repr__(self) -> str: ...
    __hash__: ClassVar[None]  # type: ignore[assignment]

# =============================================================================
# Functions — URI & Paths
# =============================================================================

def parse_uri(uri: str) -> ParsedURI:
    """Parse a URI string into components. Always succeeds — unrecognized URIs are treated as package names."""
    ...

def normalize_path(path: str) -> str:
    """
    Normalize a filesystem path (resolve ``..`` and ``.``, make absolute).

    Raises:
        UnicodeDecodeError: If the result path contains non-UTF-8 bytes.
    """
    ...

def get_amplifier_home() -> str:
    """
    Returns the Amplifier home directory.

    Resolution order: ``$AMPLIFIER_HOME`` → ``~/.amplifier`` → ``./.amplifier``.
    """
    ...

def construct_agent_path(base: str, name: str) -> str:
    """
    Returns path to ``{base}/agents/{name}.md``.

    ``.md`` is appended only if ``name`` does not already end with ``.md``.
    """
    ...

def construct_context_path(base: str, name: str) -> str:
    """Returns path to ``{base}/{name}``. Leading ``/`` stripped from name."""
    ...

# =============================================================================
# Functions — Dict Utilities
# =============================================================================

def deep_merge(base: dict[str, Any], overlay: dict[str, Any]) -> dict[str, Any]:
    """
    Deep merge two dicts. Mappings merged recursively; overlay wins for scalars and sequences.

    Raises:
        TypeError: If either argument is not a dict.
    """
    ...

def deep_merge_json(base_json: str, overlay_json: str) -> str:
    """
    Legacy JSON-string interface for deep_merge. Prefer ``deep_merge()``.

    Raises:
        ValueError: If either argument is not valid JSON.
    """
    ...

def get_nested(data: dict[str, Any], path: list[str]) -> Optional[Any]:
    """
    Traverse nested dict by key path. Returns ``None`` if path not found.

    Note: Returns a deep copy, not a reference. Only JSON-like types supported.
    """
    ...

def get_nested_with_default(data: dict[str, Any], path: list[str], default: Any) -> Any:
    """
    Like ``get_nested`` but returns ``default`` (as-is, no conversion) when path not found.

    Found values are deep-copied through YAML round-trip (same as ``get_nested``).
    """
    ...

def set_nested(data: dict[str, Any], path: list[str], value: Any) -> dict[str, Any]:
    """
    Returns a **new** dict with ``value`` set at ``path``. Does **not** mutate input.

    Creates intermediate dicts as needed. Empty path returns a copy of data.

    .. note:: Returns a deep copy. Only JSON-like types supported.
    """
    ...

# =============================================================================
# Functions — Mentions & Tracing
# =============================================================================

def parse_mentions(text: str) -> list[str]:
    """Extract @mentions from text, excluding code blocks and email addresses."""
    ...

def generate_sub_session_id(
    agent_name: Optional[str] = None,
    session_id: Optional[str] = None,
    trace_id: Optional[str] = None,
) -> str:
    """Generate a W3C Trace Context sub-session ID for agent delegation."""
    ...

# =============================================================================
# Functions — Validation
# =============================================================================

def validate_bundle(bundle: Bundle) -> ValidationResult:
    """Basic validation: required fields and module list format."""
    ...

def validate_bundle_completeness(bundle: Bundle) -> ValidationResult:
    """Strict validation: requires session, orchestrator, and providers."""
    ...

def validate_bundle_or_raise(bundle: Bundle) -> None:
    """
    Raises:
        BundleValidationError: If bundle has validation errors.
    """
    ...

def validate_bundle_completeness_or_raise(bundle: Bundle) -> None:
    """
    Raises:
        BundleValidationError: If bundle is incomplete for mounting.
    """
    ...

# =============================================================================
# Functions — Provider Preferences
# =============================================================================

def apply_provider_preferences(
    mount_plan: dict[str, Any],
    preferences: list[ProviderPreference],
) -> dict[str, Any]:
    """
    Apply provider preferences to a mount plan. Returns a new mount plan
    with the preferred provider promoted. Sync only — does NOT resolve globs.
    """
    ...

def is_glob_pattern(pattern: str) -> bool:
    """Check if a string contains glob characters (``*``, ``?``, ``[``)."""
    ...

# =============================================================================
# Functions — Serialization
# =============================================================================

def sanitize_for_json(data: Any, max_depth: Optional[int] = None) -> Any:
    """
    Recursively remove null values from dicts and lists for JSON serialization.

    Default max recursion depth: 50. Returns ``None`` at depth 0.
    """
    ...

def sanitize_message(message: Any) -> dict[str, Any]:
    """
    Sanitize a chat message for persistence.

    Handles ``thinking_block`` (extracts ``.text`` as ``thinking_text``)
    and ``content_blocks`` (skipped). Non-dict input returns ``{}``.
    """
    ...

def merge_module_lists(
    parent: list[dict[str, Any]], child: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    """
    Merge two module lists by ``"module"`` key.

    Deep-merges matching entries; appends new ones from child.

    Raises:
        TypeError: If args are not lists or elements lack a ``"module"`` key.
    """
    ...

def format_directory_listing(path: str) -> str:
    """
    Format directory contents as ``DIR name`` / ``FILE name`` lines, dirs first.

    Never raises — errors are embedded in the returned string.
    """
    ...

# =============================================================================
# Functions — IO
# =============================================================================

def parse_frontmatter(content: str) -> tuple[Optional[Any], str]:
    """
    Parse YAML frontmatter from text bounded by ``---`` delimiters.

    Returns ``(frontmatter, body)`` where frontmatter is typically a dict
    (but could be any YAML type) or ``None`` if no frontmatter found.

    Raises:
        BundleLoadError: If YAML is malformed.
        ValueError: If parsed YAML cannot be converted to a Python object.
    """
    ...

# =============================================================================
# Functions — Session Slice
# =============================================================================

def count_turns(messages: list[dict[str, Any]]) -> int:
    """Count user messages (turns) in a conversation."""
    ...

def get_turn_boundaries(messages: list[dict[str, Any]]) -> list[int]:
    """Return 0-indexed positions of each user message (turn start)."""
    ...

def slice_to_turn(
    messages: list[dict[str, Any]],
    turn: int,
    handle_orphaned_tools: Optional[Literal["complete", "remove", "error"]] = None,
) -> list[dict[str, Any]]:
    """
    Slice messages to include only up to turn N (1-indexed).

    Args:
        handle_orphaned_tools: How to handle tool calls without results.
            ``None`` or ``"complete"``: add synthetic error results (default).
            ``"remove"``: remove orphaned tool_use content blocks.
            ``"error"``: raise BundleLoadError.

    Raises:
        BundleLoadError: If turn < 1, out of range, or no user messages.
    """
    ...

def find_orphaned_tool_calls(messages: list[dict[str, Any]]) -> list[str]:
    """Return tool call IDs with no matching tool result."""
    ...

def add_synthetic_tool_results(
    messages: list[dict[str, Any]],
    orphaned_ids: list[str],
) -> list[dict[str, Any]]:
    """
    Append synthetic error tool results for orphaned tool calls.
    Returns a new list (does not mutate input).
    """
    ...

def get_turn_summary(messages: list[dict[str, Any]], turn: int) -> dict[str, Any]:
    """
    Summary dict for turn N: ``turn``, ``user_content``, ``assistant_content``,
    ``tool_count``, ``message_count``.

    Raises:
        BundleLoadError: If turn out of range or no user messages.
    """
    ...

# =============================================================================
# Functions — Session Fork
# =============================================================================

def fork_session(
    session_dir: str,
    turn: Optional[int] = None,
    new_session_id: Optional[str] = None,
    target_dir: Optional[str] = None,
    include_events: bool = True,
) -> ForkResult:
    """
    Fork a stored session at turn N. Creates a new session directory.

    Raises:
        BundleLoadError: If session_dir is invalid or turn out of range.
    """
    ...

def fork_session_in_memory(
    messages: list[dict[str, Any]],
    turn: Optional[int] = None,
    parent_id: Optional[str] = None,
) -> ForkResult:
    """
    Fork a session in-memory (no disk I/O).

    Raises:
        BundleLoadError: If no user messages or turn out of range.
    """
    ...

def get_fork_preview(session_dir: str, turn: int) -> dict[str, Any]:
    """
    Preview fork metadata without creating files.

    Raises:
        BundleLoadError: If session_dir is invalid or turn out of range.
    """
    ...

def list_session_forks(session_dir: str) -> list[dict[str, Any]]:
    """
    List all sessions forked from the given session.

    Raises:
        BundleLoadError: If session_dir is invalid.
    """
    ...

def get_session_lineage(session_dir: str) -> dict[str, Any]:
    """
    Get full lineage (ancestors + children) of a session.

    Raises:
        BundleLoadError: If session_dir is invalid.
    """
    ...

# =============================================================================
# Functions — Session Events
# =============================================================================

def count_events(events_path: str) -> int:
    """
    Count events in an events.jsonl file.
    Returns 0 if file doesn't exist — never raises.
    """
    ...

def get_event_summary(events_path: str) -> dict[str, Any]:
    """
    Event summary: ``total_events``, ``event_types``, ``first_timestamp``, ``last_timestamp``.

    Returns zero-count summary if file doesn't exist.

    Raises:
        BundleLoadError: If file exists but cannot be read/parsed.
    """
    ...