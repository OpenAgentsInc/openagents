# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
from dataclasses import dataclass


@dataclass
class Event:
    kind: str
    payload: str
