"""Base model class for all entities."""

from abc import ABC, abstractmethod
from datetime import datetime
from typing import Any, Dict, Optional


class BaseModel(ABC):
    """Abstract base class for all models."""
    
    def __init__(self, id: Optional[int] = None):
        self.id = id
        self.created_at = datetime.utcnow()
        self.updated_at = datetime.utcnow()
    
    @abstractmethod
    def to_dict(self) -> Dict[str, Any]:
        """Convert model to dictionary."""
        pass
    
    @classmethod
    @abstractmethod
    def from_dict(cls, data: Dict[str, Any]) -> "BaseModel":
        """Create model from dictionary."""
        pass
    
    def update(self, **kwargs) -> None:
        """Update model attributes."""
        for key, value in kwargs.items():
            if hasattr(self, key):
                setattr(self, key, value)
        self.updated_at = datetime.utcnow()
    
    def __eq__(self, other: object) -> bool:
        if not isinstance(other, BaseModel):
            return False
        return self.id == other.id
    
    def __hash__(self) -> int:
        return hash(self.id)
