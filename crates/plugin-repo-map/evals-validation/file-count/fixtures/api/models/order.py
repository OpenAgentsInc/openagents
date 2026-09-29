"""An order."""

from dataclasses import dataclass


@dataclass
class Order:
    id: str
    user_id: str
    total_cents: int

    def to_dict(self):
        return {"id": self.id, "user_id": self.user_id, "total_cents": self.total_cents}
