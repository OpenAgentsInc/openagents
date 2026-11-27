# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
from event import Event


def emit_event(kind: str, payload: str) -> Event:
    return Event(kind=kind, payload=payload)
