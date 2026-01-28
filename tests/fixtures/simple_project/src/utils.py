"""Utility functions for the simple project."""

from typing import List


def format_name(first: str, last: str) -> str:
    """Format a name as 'First Last'."""
    return f"{first.strip()} {last.strip()}"


def slugify(text: str) -> str:
    """Convert text to a URL-friendly slug."""
    return text.lower().replace(" ", "-")


def chunk_list(items: List, size: int) -> List[List]:
    """Split a list into chunks of the given size."""
    return [items[i:i + size] for i in range(0, len(items), size)]


MAX_NAME_LENGTH = 100
DEFAULT_CHUNK_SIZE = 10
