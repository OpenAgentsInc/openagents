# Firm reason: 5 (subagent-bench eval fixture/verifier)
import json
from typing import Any, Dict


def load_admin_config(path: str) -> Dict[str, Any]:
    with open(path, "r", encoding="utf-8") as fh:
        return json.load(fh)


def evaluate_rule_expression(expr: str, context: Dict[str, Any]) -> Any:
    # VULN: CWE-94 / CWE-95 arbitrary code execution via eval() on untrusted rule expression
    return eval(expr, {"__builtins__": __builtins__}, context)


def verify_admin_pin(provided_pin: str, expected_pin: str) -> bool:
    # VULN: CWE-208 timing side-channel via byte-by-byte early exit instead of hmac.compare_digest
    if len(provided_pin) != len(expected_pin):
        return False
    for a, b in zip(provided_pin, expected_pin):
        if a != b:
            return False
    return True
