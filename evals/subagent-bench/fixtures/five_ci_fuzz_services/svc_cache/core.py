# Firm reason: 5 (subagent-bench eval fixture/verifier)
from collections import OrderedDict
from typing import Any, Optional


class TTLLRUCache:
    def __init__(self, capacity: int):
        if capacity <= 0:
            raise ValueError("capacity must be > 0")
        self.capacity = capacity
        self._data: "OrderedDict[str, tuple[Any, int]]" = OrderedDict()

    def put(self, key: str, value: Any, now_s: int, ttl_s: int) -> None:
        if ttl_s <= 0:
            raise ValueError("ttl_s must be > 0")
        # BUG 1: does not purge expired entries before checking capacity, AND does not move updated key to MRU!
        if key in self._data:
            self._data[key] = (value, now_s + ttl_s)
            return
        if len(self._data) >= self.capacity:
            self._data.popitem(last=False)
        self._data[key] = (value, now_s + ttl_s)

    def get(self, key: str, now_s: int) -> Optional[Any]:
        item = self._data.get(key)
        if item is None:
            return None
        val, exp = item
        if now_s >= exp:
            del self._data[key]
            return None
        # BUG 2: forgets to mark key as most-recently-used on get()!
        return val
