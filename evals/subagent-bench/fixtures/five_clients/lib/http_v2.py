# Firm reason: 5 (subagent-bench eval fixture/verifier)
"""Canonical v2 typed HTTP client with Envelope, typed ApiErrors, idempotency, and cursor pagination."""
from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional, Tuple


class ApiError(RuntimeError):
    def __init__(self, code: int, message: str):
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message


class NotFoundError(ApiError):
    pass


class RateLimitError(ApiError):
    pass


class ConflictError(ApiError):
    pass


@dataclass(frozen=True)
class Envelope:
    status: int
    data: Any
    next_cursor: Optional[str] = None
    request_id: str = "req_v2"


class HttpV2Client:
    def __init__(self, routes: Dict[Tuple[str, str], Callable[[Any, Dict[str, str]], Envelope]]):
        self.routes = routes
        self.calls: List[Tuple[str, str, Any, Dict[str, str]]] = []

    def send(
        self,
        method: str,
        path: str,
        body: Any = None,
        headers: Optional[Dict[str, str]] = None,
        idempotency_key: Optional[str] = None,
    ) -> Envelope:
        hdrs = dict(headers or {})
        if method in ("POST", "PUT", "PATCH") and idempotency_key:
            hdrs["Idempotency-Key"] = idempotency_key
        self.calls.append((method, path, body, hdrs))
        handler = self.routes.get((method, path))
        if handler is None:
            raise NotFoundError(404, f"unknown route {method} {path}")
        env = handler(body, hdrs)
        if env.status == 404:
            raise NotFoundError(404, str(env.data))
        if env.status == 409:
            raise ConflictError(409, str(env.data))
        if env.status == 429:
            raise RateLimitError(429, str(env.data))
        if env.status >= 400:
            raise ApiError(env.status, str(env.data))
        return env

    def paginate(self, path: str) -> List[Any]:
        out: List[Any] = []
        cursor: Optional[str] = None
        while True:
            hdrs = {"X-Cursor": cursor} if cursor else {}
            env = self.send("GET", path, headers=hdrs)
            if isinstance(env.data, list):
                out.extend(env.data)
            if not env.next_cursor:
                break
            cursor = env.next_cursor
        return out
