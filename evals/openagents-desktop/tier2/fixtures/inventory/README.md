# inventory

`restock(items, name, qty)` adds `qty` of `name` to the stock dict (creating the entry when
missing) and returns the dict. A `qty` below 1 raises `ValueError`. It is not written yet.
