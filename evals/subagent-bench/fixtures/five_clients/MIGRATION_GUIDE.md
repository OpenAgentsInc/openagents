# HTTP v1 -> v2 Client Migration Guide

Migrate every client package under `clients/` (`orders`, `users`, `catalog`, `shipping`, `ledger`) from `lib.http_v1.LegacyTransport` to `lib.http_v2.HttpV2Client`:

1. **Zero `http_v1` imports or references** may remain anywhere under `clients/`.
2. Each client class must accept `http: HttpV2Client` in `__init__(self, http: HttpV2Client)`.
3. **Reads (`get_*`)**: call `self.http.send("GET", path)` and return `env.data`; let `NotFoundError` / `ApiError` propagate (do NOT return `None` or raw dicts with `ok: False`).
4. **Mutating writes (`create_*` / `record_*` / `dispatch_*`)**: accept `idempotency_key: str` (keyword or positional, required non-empty; raise `ValueError` if empty) and pass `idempotency_key=idempotency_key` to `self.http.send("POST", path, body=..., idempotency_key=idempotency_key)`, returning `env.data`.
5. **List methods (`list_*`)**: use `self.http.paginate(path)` to return all items across pages.
6. Update each package's `test_*.py` to test against `HttpV2Client` + `Envelope`, covering `get_*` (success + `NotFoundError`), `create_*`/`record_*`/`dispatch_*` (verifying `Idempotency-Key` header + empty-key `ValueError`), and multi-page `list_*` pagination.
