# Firm reason: 5 (subagent-bench eval fixture/verifier)
from typing import Any, Dict, List, Optional
from lib.http_v1 import LegacyTransport


class OrdersClient:
    def __init__(self, transport: LegacyTransport):
        self.transport = transport

    def get_order(self, order_id: str) -> Optional[Dict[str, Any]]:
        res = self.transport.request("GET", f"/v2/orders/{order_id}")
        if not res.get(ok):
            return None
        return res["data"]

    def create_order(self, payload: Dict[str, Any]) -> Dict[str, Any]:
        res = self.transport.request("POST", "/v2/orders", body=payload)
        if not res.get(ok):
            raise RuntimeError(res.get("err", "failed"))
        return res["data"]

    def list_orders(self) -> List[Dict[str, Any]]:
        res = self.transport.request("GET", "/v2/orders")
        if not res.get(ok):
            return []
        return list(res["data"])
