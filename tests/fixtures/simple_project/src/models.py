"""Data models for the simple project."""

from dataclasses import dataclass
from typing import Optional


@dataclass
class User:
    """Represents a user in the system."""
    
    first_name: str
    last_name: str
    email: Optional[str] = None
    
    def full_name(self) -> str:
        """Return the user's full name."""
        return f"{self.first_name} {self.last_name}"
    
    def __repr__(self) -> str:
        return f"User({self.first_name!r}, {self.last_name!r})"


class Admin(User):
    """An admin user with extra privileges."""
    
    def __init__(self, first_name: str, last_name: str, role: str = "admin"):
        super().__init__(first_name, last_name)
        self.role = role
    
    def can_delete(self) -> bool:
        """Check if admin can delete resources."""
        return self.role == "admin"
