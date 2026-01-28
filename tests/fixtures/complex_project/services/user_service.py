"""User service for business logic."""

from typing import Dict, List, Optional
from ..models.user import User, UserRole
from ..utils.validators import validate_email


class UserService:
    """Service for user-related operations."""
    
    def __init__(self):
        self._users: Dict[int, User] = {}
        self._next_id = 1
    
    def create(self, data: Dict) -> User:
        """Create a new user."""
        if not validate_email(data.get("email", "")):
            raise ValueError("Invalid email address")
        
        user = User(
            id=self._next_id,
            username=data["username"],
            email=data["email"],
            role=UserRole(data.get("role", "user")),
        )
        
        if "password" in data:
            user.set_password(data["password"])
        
        self._users[user.id] = user
        self._next_id += 1
        
        return user
    
    def get_by_id(self, user_id: int) -> Optional[User]:
        """Get user by ID."""
        return self._users.get(user_id)
    
    def get_by_email(self, email: str) -> Optional[User]:
        """Get user by email address."""
        for user in self._users.values():
            if user.email == email:
                return user
        return None
    
    def get_all(self, active_only: bool = True) -> List[User]:
        """Get all users."""
        users = list(self._users.values())
        if active_only:
            users = [u for u in users if u.active]
        return users
    
    def update(self, user_id: int, data: Dict) -> Optional[User]:
        """Update a user."""
        user = self.get_by_id(user_id)
        if not user:
            return None
        
        if "email" in data and not validate_email(data["email"]):
            raise ValueError("Invalid email address")
        
        user.update(**data)
        return user
    
    def delete(self, user_id: int) -> bool:
        """Delete a user (soft delete)."""
        user = self.get_by_id(user_id)
        if not user:
            return False
        
        user.active = False
        return True
    
    def count(self, active_only: bool = True) -> int:
        """Count users."""
        return len(self.get_all(active_only))
