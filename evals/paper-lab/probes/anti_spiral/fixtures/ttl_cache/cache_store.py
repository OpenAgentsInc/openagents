# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
"""Bounded TTL cache with deterministic injectable clock."""
from evictor import pick_victim


class TTLCache:
    def __init__(self, capacity: int, default_ttl: float, clock):
        self.capacity = capacity
        self.default_ttl = default_ttl
        self.clock = clock
        self._entries: dict[str, tuple[str, float, float]] = {}

    def _purge_expired(self, now: float) -> None:
        # BUG: uses strict '>' instead of '>=' so entries expiring exactly at `now` stay alive
        expired = [k for k, (_, exp, _) in self._entries.items() if now > exp]
        for k in expired:
            del self._entries[k]

    def put(self, key: str, value: str, ttl: float | None = None) -> None:
        now = self.clock()
        self._purge_expired(now)
        eff_ttl = self.default_ttl if ttl is None else ttl
        if key not in self._entries and len(self._entries) >= self.capacity:
            victim = pick_victim(self._entries)
            if victim is not None:
                del self._entries[victim]
        self._entries[key] = (value, now + eff_ttl, now)

    def get(self, key: str) -> str | None:
        now = self.clock()
        self._purge_expired(now)
        item = self._entries.get(key)
        if item is None:
            return None
        val, exp, _ = item
        # BUG: slides expires_at forward on read instead of keeping absolute TTL (should only update last_used_at)
        self._entries[key] = (val, now + self.default_ttl, now)
        return val
