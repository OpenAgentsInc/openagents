"""A user."""

from dataclasses import dataclass


@dataclass
class User:
    id: str
    name: str

    def to_dict(self):
        return {"id": self.id, "name": self.name}
