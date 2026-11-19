# lru

`LRUCache(capacity)` keeps at most `capacity` keys.

- `get(key)` returns the value or `None`, and marks the key as most recently used.
- `put(key, value)` inserts or updates the key and marks it most recently used; when the cache is over capacity it evicts the least recently used key.
- `len(cache)` is the number of keys held.
