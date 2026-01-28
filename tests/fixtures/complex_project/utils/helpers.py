"""General helper functions."""

import re
from typing import Any, List, Optional


def slugify(text: str, separator: str = "-") -> str:
    """
    Convert text to URL-friendly slug.
    
    Args:
        text: The text to slugify
        separator: Character to use between words
        
    Returns:
        URL-friendly slug
    """
    # Convert to lowercase
    text = text.lower()
    
    # Replace non-alphanumeric with separator
    text = re.sub(r"[^a-z0-9]+", separator, text)
    
    # Remove leading/trailing separators
    text = text.strip(separator)
    
    return text


def truncate(text: str, max_length: int = 100, suffix: str = "...") -> str:
    """
    Truncate text to a maximum length.
    
    Args:
        text: The text to truncate
        max_length: Maximum length including suffix
        suffix: String to append if truncated
        
    Returns:
        Truncated text
    """
    if len(text) <= max_length:
        return text
    
    return text[:max_length - len(suffix)] + suffix


def chunk_list(items: List[Any], size: int) -> List[List[Any]]:
    """
    Split a list into chunks.
    
    Args:
        items: List to split
        size: Maximum chunk size
        
    Returns:
        List of chunks
    """
    return [items[i:i + size] for i in range(0, len(items), size)]


def safe_get(obj: Any, *keys: str, default: Any = None) -> Any:
    """
    Safely get nested dictionary values.
    
    Args:
        obj: The object to traverse
        *keys: Keys to access
        default: Default value if not found
        
    Returns:
        The value or default
    """
    for key in keys:
        if isinstance(obj, dict):
            obj = obj.get(key)
        else:
            return default
        if obj is None:
            return default
    return obj


def flatten(nested: List[List[Any]]) -> List[Any]:
    """Flatten a nested list."""
    return [item for sublist in nested for item in sublist]


# Constants
MAX_SLUG_LENGTH = 100
DEFAULT_PAGE_SIZE = 20
