"""Validation utility functions."""

import re
from typing import Optional


EMAIL_REGEX = re.compile(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
USERNAME_REGEX = re.compile(r"^[a-zA-Z][a-zA-Z0-9_]{2,29}$")


def validate_email(email: str) -> bool:
    """
    Validate an email address.
    
    Args:
        email: The email address to validate
        
    Returns:
        True if valid, False otherwise
    """
    if not email or not isinstance(email, str):
        return False
    return bool(EMAIL_REGEX.match(email))


def validate_username(username: str) -> bool:
    """
    Validate a username.
    
    Requirements:
    - Start with a letter
    - 3-30 characters
    - Only letters, numbers, underscores
    
    Args:
        username: The username to validate
        
    Returns:
        True if valid, False otherwise
    """
    if not username or not isinstance(username, str):
        return False
    return bool(USERNAME_REGEX.match(username))


def validate_password(password: str, min_length: int = 8) -> Optional[str]:
    """
    Validate a password and return error message if invalid.
    
    Args:
        password: The password to validate
        min_length: Minimum required length
        
    Returns:
        Error message if invalid, None if valid
    """
    if not password:
        return "Password is required"
    
    if len(password) < min_length:
        return f"Password must be at least {min_length} characters"
    
    if not any(c.isupper() for c in password):
        return "Password must contain an uppercase letter"
    
    if not any(c.islower() for c in password):
        return "Password must contain a lowercase letter"
    
    if not any(c.isdigit() for c in password):
        return "Password must contain a digit"
    
    return None
