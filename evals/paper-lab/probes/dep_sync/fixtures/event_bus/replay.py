# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
import json
from event import Event


def serialize_event(ev: Event) -> str:
    return json.dumps({"kind": ev.kind, "payload": ev.payload})


def deserialize_event(raw: str) -> Event:
    data = json.loads(raw)
    return Event(kind=data["kind"], payload=data["payload"])
