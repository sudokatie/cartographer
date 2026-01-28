"""Main entry point for the simple project."""

from .models import User
from .utils import format_name


def main():
    """Run the main application."""
    user = User("John", "Doe")
    print(format_name(user.first_name, user.last_name))


if __name__ == "__main__":
    main()
