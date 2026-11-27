from event import Event


def emit_event(kind: str, payload: str) -> Event:
    return Event(kind=kind, payload=payload)
