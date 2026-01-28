"""API middleware for request processing."""

from typing import Callable, Dict, Optional
import time


class Middleware:
    """Base middleware class."""
    
    def __init__(self, next_handler: Optional[Callable] = None):
        self.next_handler = next_handler
    
    async def __call__(self, request: Dict) -> Dict:
        """Process the request."""
        raise NotImplementedError


class AuthMiddleware(Middleware):
    """Authentication middleware."""
    
    def __init__(self, secret_key: str, **kwargs):
        super().__init__(**kwargs)
        self.secret_key = secret_key
    
    async def __call__(self, request: Dict) -> Dict:
        """Validate authentication token."""
        token = request.get("headers", {}).get("Authorization")
        
        if not token:
            return {"error": "Unauthorized", "status": 401}
        
        if not self._validate_token(token):
            return {"error": "Invalid token", "status": 403}
        
        if self.next_handler:
            return await self.next_handler(request)
        return request
    
    def _validate_token(self, token: str) -> bool:
        """Validate the authentication token."""
        # Simplified validation
        return token.startswith("Bearer ")


class LoggingMiddleware(Middleware):
    """Request logging middleware."""
    
    async def __call__(self, request: Dict) -> Dict:
        """Log the request and response."""
        start = time.time()
        
        if self.next_handler:
            response = await self.next_handler(request)
        else:
            response = request
        
        duration = time.time() - start
        print(f"Request processed in {duration:.3f}s")
        
        return response


def auth_middleware(secret: str) -> AuthMiddleware:
    """Factory function for auth middleware."""
    return AuthMiddleware(secret)
