# Firm reason: 5 (subagent-bench eval fixture/verifier)
from typing import Any, Dict, List, Optional
from lib.http_v1 import LegacyTransport


class LedgerClient:
    def __init__(self, transport: LegacyTransport):
        self.transport = transport

    def get_entry(self, entry_id: str) -> Optional[Dict[str, Any]]:
        res = self.transport.request("GET", f"/v2/ledger/{entry_id}")
        if not res.get(ok):
            return None
        return res["data"]

    def record_entry(self, payload: Dict[str, Any]) -> Dict[str, Any]:
        res = self.transport.request("POST", "/v2/ledger", body=payload)
        if not res.get(ok):
            raise RuntimeError(res.get("err", "failed"))
        return res["data"]

    def list_entries(self) -> List[Dict[str, Any]]:
        res = self.transport.request("GET", "/v2/ledger")
        if not res.get(ok):
            return []
        return list(res["data"])
