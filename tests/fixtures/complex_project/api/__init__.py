"""API module for handling HTTP requests."""

from .routes import router
from .middleware import auth_middleware

__all__ = ["router", "auth_middleware"]
