#!/usr/bin/env python3
# Firm reason: 5 (subagent-bench eval fixture/verifier)
import json
import re
from pathlib import Path

rep_path = Path("AUDIT_REPORT.json")
assert rep_path.is_file(), "AUDIT_REPORT.json missing"
data = json.loads(rep_path.read_text(encoding="utf-8"))
findings = data.get("findings")
assert isinstance(findings, list) and len(findings) >= 10, f"too few findings: {len(findings) if isinstance(findings, list) else type(findings)}"

# Expected 12 (service, accepted_cwes, approx_line)
EXPECTED = [
    ("gateway",  {"798", "259", "321"}, 10),
    ("gateway",  {"601"}, 30),
    ("payments", {"89"}, 12),
    ("payments", {"639", "284", "862", "863"}, 19),
    ("uploads",  {"22", "23", "73"}, 16),
    ("uploads",  {"78", "77"}, 23),
    ("search",   {"1333", "400", "185"}, 12),
    ("search",   {"79", "80"}, 19),
    ("webhooks", {"918"}, 9),
    ("webhooks", {"502"}, 16),
    ("admin",    {"94", "95"}, 12),
    ("admin",    {"208", "203"}, 16),
]

def norm_cwe(raw) -> str:
    m = re.search(r"(\d+)", str(raw or ""))
    return m.group(1) if m else ""

matched = 0
services_hit = set()
for svc, cwes, target_line in EXPECTED:
    hit = False
    for f in findings:
        f_svc = str(f.get("service", "")).lower()
        f_file = str(f.get("file", "")).lower()
        f_cwe = norm_cwe(f.get("cwe", ""))
        f_line = int(f.get("line", 0) or 0)
        if (svc in f_svc or svc in f_file) and f_cwe in cwes and abs(f_line - target_line) <= 12:
            hit = True
            services_hit.add(svc)
            break
    if hit:
        matched += 1

assert len(services_hit) == 6, f"expected all 6 services covered, got {sorted(services_hit)}"
assert matched >= 10, f"expected >=10/12 exact findings, matched {matched}/12"
print(f"SB3_AUDIT_OK matched={matched}/12 services={len(services_hit)}/6")
