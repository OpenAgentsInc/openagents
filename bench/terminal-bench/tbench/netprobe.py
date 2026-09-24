"""A model-free agent that probes the agent phase's network (issue #9589).

``NetProbe`` runs no model. During the agent phase it requests each of the
agent's allowed hosts and a few hosts that aren't allowed, from inside the
task container, and writes ``network-probe.json`` beside its logs: the
policy the environment applied, each request's result, and whether the
allowlist held. A request counts as reaching a host when any HTTP status
comes back, a 403 or a 404 included; a connection the egress proxy
refuses returns none. Plain TCP connects prove nothing here, because the
transparent proxy accepts every connection before it reads the host name.
"""

from __future__ import annotations

import json
import shlex
from typing import Any

from harbor.agents.base import BaseAgent
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext

PROBE_NAME = "network-probe.json"
PROBE_SCHEMA = "openagents.tbench.network-probe.v1"
# Hosts no Coder One arm needs: a package index, a code host, a generic
# site, and an address with no host name at all.
BLOCKED_URLS = (
    "https://example.com/",
    "https://pypi.org/simple/",
    "https://github.com/",
    "http://1.1.1.1/",
)

# curl first, then Python's urllib; each prints `tool url status exit detail`.
_PROBE = r"""
probe() {
  url="$1"
  if command -v curl >/dev/null 2>&1; then
    status=$(curl -sS -o /dev/null -w '%{http_code}' --connect-timeout 8 --max-time 15 "$url" 2>/tmp/netprobe.err)
    code=$?
    echo "curl $url ${status:-000} $code $(head -c 160 /tmp/netprobe.err | tr '\n' ' ')"
  elif command -v python3 >/dev/null 2>&1; then
    python3 - "$url" <<'PY'
import sys, urllib.error, urllib.request
url = sys.argv[1]
try:
    with urllib.request.urlopen(url, timeout=15) as response:
        print("python3", url, response.status, 0, "")
except urllib.error.HTTPError as error:
    print("python3", url, error.code, 0, "")
except Exception as error:
    print("python3", url, "000", 1, type(error).__name__, str(error)[:120].replace("\n", " "))
PY
  else
    echo "none $url 000 127 no curl or python3 in the image"
  fi
}
"""


def parse_probe(stdout: str) -> list[dict[str, Any]]:
    """The probe script's lines as records."""
    results = []
    for line in stdout.splitlines():
        parts = line.split(" ", 4)
        if len(parts) < 4 or parts[0] not in ("curl", "python3", "none"):
            continue
        status = parts[2]
        results.append(
            {
                "tool": parts[0],
                "url": parts[1],
                "status": status,
                "exit": int(parts[3]) if parts[3].lstrip("-").isdigit() else None,
                "detail": parts[4].strip() if len(parts) > 4 else "",
                "reached": status.isdigit() and status != "000",
            }
        )
    return results


def verdict(policy: dict[str, Any], results: list[dict[str, Any]]) -> dict[str, Any]:
    """Whether the allowlist held: every allowed host reached, no other."""
    allowed = [r for r in results if r["expected"] == "reachable"]
    blocked = [r for r in results if r["expected"] == "unreachable"]
    leaked = [r["url"] for r in blocked if r["reached"]]
    missed = [r["url"] for r in allowed if not r["reached"]]
    return {
        "allowlist_enforced": policy.get("network_mode") == "allowlist" and not leaked,
        "reached_allowed": len(allowed) - len(missed),
        "allowed": len(allowed),
        "leaked": leaked,
        "missed": missed,
    }


class NetProbe(BaseAgent):
    """Probe the agent phase's egress; no model, no spend."""

    @staticmethod
    def name() -> str:
        return "netprobe"

    def version(self) -> str:
        return "1.0.0"

    async def setup(self, environment: BaseEnvironment) -> None:
        return None

    async def run(
        self, instruction: str, environment: BaseEnvironment, context: AgentContext
    ) -> None:
        policy_obj = environment.network_policy
        policy = {
            "network_mode": policy_obj.network_mode.value,
            "allowed_hosts": list(policy_obj.allowed_hosts),
        }
        allowed = [f"https://{host}/" for host in policy["allowed_hosts"]]
        urls = [(url, "reachable") for url in allowed] + [
            (url, "unreachable") for url in BLOCKED_URLS
        ]
        script = _PROBE + "\n".join(f"probe {shlex.quote(url)}" for url, _ in urls)
        result = await environment.exec(command=f"sh -c {shlex.quote(script)}", timeout_sec=180)
        expected = dict(urls)
        results = parse_probe(result.stdout or "")
        for row in results:
            row["expected"] = expected.get(row["url"], "unknown")
        body = {
            "schema": PROBE_SCHEMA,
            "policy": policy,
            "results": results,
            "verdict": verdict(policy, results),
            "stderr": (result.stderr or "")[-2000:],
        }
        (self.logs_dir / PROBE_NAME).write_text(json.dumps(body, indent=2) + "\n")
        context.metadata = {**(context.metadata or {}), "network_probe": body["verdict"]}
