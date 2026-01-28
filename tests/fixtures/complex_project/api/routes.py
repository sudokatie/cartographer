"""API route definitions."""

from typing import Dict, List, Optional
from ..models.user import User
from ..services.user_service import UserService


class Router:
    """HTTP router for handling API requests."""
    
    def __init__(self):
        self.routes: Dict[str, callable] = {}
        self.user_service = UserService()
    
    def get(self, path: str):
        """Decorator for GET routes."""
        def decorator(func):
            self.routes[f"GET:{path}"] = func
            return func
        return decorator
    
    def post(self, path: str):
        """Decorator for POST routes."""
        def decorator(func):
            self.routes[f"POST:{path}"] = func
            return func
        return decorator
    
    async def handle(self, method: str, path: str, **kwargs) -> Dict:
        """Handle an incoming request."""
        key = f"{method}:{path}"
        if key not in self.routes:
            return {"error": "Not found", "status": 404}
        
        handler = self.routes[key]
        return await handler(**kwargs)


router = Router()


@router.get("/users")
async def list_users() -> List[Dict]:
    """List all users."""
    users = router.user_service.get_all()
    return [u.to_dict() for u in users]


@router.get("/users/{id}")
async def get_user(id: int) -> Optional[Dict]:
    """Get a user by ID."""
    user = router.user_service.get_by_id(id)
    return user.to_dict() if user else None


@router.post("/users")
async def create_user(data: Dict) -> Dict:
    """Create a new user."""
    user = router.user_service.create(data)
    return user.to_dict()
