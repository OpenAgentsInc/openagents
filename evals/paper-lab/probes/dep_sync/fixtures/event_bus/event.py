from dataclasses import dataclass


@dataclass
class Event:
    kind: str
    payload: str
