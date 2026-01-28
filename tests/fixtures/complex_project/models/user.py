"""User model definition."""

from enum import Enum
from typing import Any, Dict, Optional
from .base import BaseModel


class UserRole(Enum):
    """User role enumeration."""
    GUEST = "guest"
    USER = "user"
    ADMIN = "admin"
    SUPERADMIN = "superadmin"


class User(BaseModel):
    """User entity representing a system user."""
    
    def __init__(
        self,
        username: str,
        email: str,
        role: UserRole = UserRole.USER,
        id: Optional[int] = None,
        active: bool = True,
    ):
        super().__init__(id)
        self.username = username
        self.email = email
        self.role = role
        self.active = active
        self._password_hash: Optional[str] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert user to dictionary."""
        return {
            "id": self.id,
            "username": self.username,
            "email": self.email,
            "role": self.role.value,
            "active": self.active,
            "created_at": self.created_at.isoformat(),
            "updated_at": self.updated_at.isoformat(),
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "User":
        """Create user from dictionary."""
        return cls(
            id=data.get("id"),
            username=data["username"],
            email=data["email"],
            role=UserRole(data.get("role", "user")),
            active=data.get("active", True),
        )
    
    def set_password(self, password: str) -> None:
        """Set user password (hashed)."""
        # In reality, this would hash the password
        self._password_hash = f"hashed:{password}"
    
    def check_password(self, password: str) -> bool:
        """Verify password against stored hash."""
        return self._password_hash == f"hashed:{password}"
    
    def is_admin(self) -> bool:
        """Check if user has admin privileges."""
        return self.role in (UserRole.ADMIN, UserRole.SUPERADMIN)
    
    def __repr__(self) -> str:
        return f"User(id={self.id}, username={self.username!r}, role={self.role.name})"
