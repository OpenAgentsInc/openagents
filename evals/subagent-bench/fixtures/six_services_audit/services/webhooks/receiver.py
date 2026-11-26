# Firm reason: 5 (subagent-bench eval fixture/verifier)
import json
import pickle
import urllib.request
from typing import Any, Dict


def fetch_callback_metadata(callback_url: str) -> Dict[str, Any]:
    # VULN: CWE-918 SSRF - fetches arbitrary URL including internal 169.254.169.254 / localhost without allowlist
    with urllib.request.urlopen(callback_url, timeout=5) as resp:
        data = resp.read(65536)
    return json.loads(data.decode("utf-8"))


def restore_delivery_state(blob: bytes) -> Any:
    # VULN: CWE-502 unsafe pickle deserialization of untrusted bytes
    return pickle.loads(blob)


def redact_headers(headers: Dict[str, str]) -> Dict[str, str]:
    return {k: ("***" if "auth" in k.lower() or "token" in k.lower() else v) for k, v in headers.items()}
