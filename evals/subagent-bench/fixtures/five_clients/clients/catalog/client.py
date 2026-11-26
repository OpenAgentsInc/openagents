# Firm reason: 5 (subagent-bench eval fixture/verifier)
from typing import Any, Dict, List, Optional
from lib.http_v1 import LegacyTransport


class CatalogClient:
    def __init__(self, transport: LegacyTransport):
        self.transport = transport

    def get_item(self, sku: str) -> Optional[Dict[str, Any]]:
        res = self.transport.request("GET", f"/v2/catalog/{sku}")
        if not res.get(ok):
            return None
        return res["data"]

    def create_item(self, payload: Dict[str, Any]) -> Dict[str, Any]:
        res = self.transport.request("POST", "/v2/catalog", body=payload)
        if not res.get(ok):
            raise RuntimeError(res.get("err", "failed"))
        return res["data"]

    def list_items(self) -> List[Dict[str, Any]]:
        res = self.transport.request("GET", "/v2/catalog")
        if not res.get(ok):
            return []
        return list(res["data"])
