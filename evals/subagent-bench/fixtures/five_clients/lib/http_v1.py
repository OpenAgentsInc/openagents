# Firm reason: 5 (subagent-bench eval fixture/verifier)
"""DEPRECATED v1 HTTP transport. Do NOT import in clients/ after migration."""
from typing import Any, Dict


class LegacyTransport:
    def __init__(self, routes: Dict[str, Any]):
        self.routes = routes
        self.calls = []

    def request(self, method: str, path: str, body: Any = None) -> Dict[str, Any]:
        self.calls.append((method, path, body))
        handler = self.routes.get((method, path))
        if handler is None:
            return {ok: False, "err": "not_found", "code": 404}
        return handler(body)
