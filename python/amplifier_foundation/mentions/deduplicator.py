"""Content deduplication for @mentioned files."""

from __future__ import annotations

import hashlib
from pathlib import Path

from .models import ContextFile


class ContentDeduplicator:
    """Deduplicate content by SHA-256 hash with multi-path attribution.

    Tracks files that have been added and returns only unique content.
    When the same content is found at multiple paths, all paths are tracked
    so users/models know all @mentions that resolved to this content.

    Useful when loading recursive @mentions to avoid including
    the same content multiple times while crediting all sources.
    """

    def __init__(self) -> None:
        """Initialize deduplicator."""
        self._content_by_hash: dict[str, str] = {}
        self._paths_by_hash: dict[str, list[Path]] = {}

    def add_file(self, path: Path, content: str) -> bool:
        """Add a file, tracking its path even if content is duplicate.

        Args:
            path: Path to the file.
            content: File content.

        Returns:
            True if file was added (new content), False if duplicate content
            (but path is still tracked for attribution).
        """
        content_hash = hashlib.sha256(content.encode()).hexdigest()

        if content_hash not in self._content_by_hash:
            # New content
            self._content_by_hash[content_hash] = content
            self._paths_by_hash[content_hash] = [path]
            return True

        # Duplicate content - add path if not already tracked
        if path not in self._paths_by_hash[content_hash]:
            self._paths_by_hash[content_hash].append(path)
        return False

    def get_unique_files(self) -> list[ContextFile]:
        """Get list of unique files with all paths where each was found.

        Returns:
            List of ContextFile instances, one per unique content,
            each with all paths where that content was found.
        """
        return [
            ContextFile(
                content=content,
                content_hash=content_hash,
                paths=self._paths_by_hash[content_hash],
            )
            for content_hash, content in self._content_by_hash.items()
        ]

    def is_seen(self, content: str) -> bool:
        """Check if content has already been seen.

        Args:
            content: Content to check.

        Returns:
            True if content hash has been seen.
        """
        content_hash = hashlib.sha256(content.encode()).hexdigest()
        return content_hash in self._content_by_hash

    def get_known_hashes(self) -> set[str]:
        """Return hashes currently tracked by the deduplicator.

        Returns:
            Set of SHA-256 hash strings for all seen content.
        """
        return set(self._content_by_hash.keys())
