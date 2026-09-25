"""Hold a multi-service task's agent behind Harbor's egress sidecar (#9607).

Under a network allowlist, Harbor 0.22.0's Docker environment writes a
compose overlay that puts ``main`` and every task service without its own
networking into the egress-control sidecar's network namespace
(``network_mode: service:<sidecar>``). Docker refuses a service in that
mode that declares ``expose:`` or ``ports:`` ("conflicting options: port
exposing and the container type network mode"), so every task whose
services expose a port failed before its agent started. Sharing one
namespace also loses the services' host names and lets two services
collide on a port.

``overlay`` builds the replacement overlay:

- Only the agent's namespace goes behind the sidecar: ``main``, or the
  service ``main`` shares a namespace with (``network_mode:
  service:<name>``). Its ``expose:`` and ``ports:`` are reset, because
  in the sidecar's namespace they publish nothing another container
  needs. The sidecar carries that service's name as a network alias, so
  the other services still reach it by name.
- Every other service keeps its own namespace on the task's networks,
  and every one of those networks becomes ``internal``: no route off the
  host, and no address on the host's side of the bridge. The services
  reach each other and the agent, and nothing else.
- The sidecar joins the task's default network and a separate egress
  network, its only route out. Before the sidecar's own entrypoint runs,
  a wrapper marks traffic to the internal networks' subnets with the
  mark the sidecar's proxy uses for its own sockets, so that traffic
  bypasses the proxy and its allowlist. Every other TCP connection goes
  through the proxy, which admits only the allowed hosts.

The result: the agent reaches the allowed hosts and the task's own
services by name, and nothing else. The services reach nothing outside
the task in any phase.
"""

from __future__ import annotations

from typing import Any

import yaml

MAIN = "main"
SIDECAR = "harbor-docker-egress-control-sidecar"
EGRESS_NETWORK = "tbench-egress"
DEFAULT_NETWORK = "default"
# Must match GOST_MARK in the sidecar's bin/network-policy: its nftables
# rules let packets with this mark skip the transparent proxy.
PROXY_BYPASS_MARK = 114514
SIDECAR_ENTRYPOINT = "/opt/egress-sidecar/entrypoint.sh"
BYPASS_TABLE = "tbench_task_services"

# Marks traffic to every subnet the sidecar reaches without a gateway,
# except the egress network's (the one carrying the default route), then
# hands over to the sidecar's own entrypoint.
_WRAPPER = f"""set -eu
egress_dev=$(ip -4 route show default | awk '{{for (i = 1; i < NF; i++) if ($i == "dev") {{ print $(i + 1); exit }}}}')
if [ -z "$egress_dev" ]; then
  echo "tbench: the egress sidecar has no default route" >&2
  exit 2
fi
subnets=$(ip -4 route show scope link | awk -v egress="$egress_dev" '{{for (i = 1; i < NF; i++) if ($i == "dev" && $(i + 1) != egress) print $1}}')
if [ -z "$subnets" ]; then
  echo "tbench: the egress sidecar is on no task network" >&2
  exit 2
fi
nft add table inet {BYPASS_TABLE}
nft add chain inet {BYPASS_TABLE} output '{{ type filter hook output priority -250; policy accept; }}'
for subnet in $subnets; do
  nft add rule inet {BYPASS_TABLE} output ip daddr "$subnet" meta mark set {PROXY_BYPASS_MARK}
  echo "tbench: task network $subnet bypasses the egress proxy"
done
exec {SIDECAR_ENTRYPOINT}
"""


class ComposeRefused(ValueError):
    """A task's compose files can't be held to the allowlist this way."""


class _Reset(list):
    """A compose ``!reset`` value: drop what earlier files declared."""


class _Dumper(yaml.SafeDumper):
    pass


_Dumper.add_representer(
    _Reset, lambda dumper, value: dumper.represent_sequence("!reset", list(value))
)


def merged_services(documents: list[Any]) -> dict[str, dict[str, Any]]:
    """The networking keys of each service across the task's compose files.

    Later files override earlier ones, as ``docker compose -f`` does for
    scalar keys; only the keys this module reads are kept.
    """
    services: dict[str, dict[str, Any]] = {}
    for document in documents:
        if not isinstance(document, dict) or not isinstance(document.get("services"), dict):
            continue
        for name, config in document["services"].items():
            if not isinstance(name, str):
                continue
            entry = services.setdefault(name, {})
            if isinstance(config, dict):
                for key in ("network_mode", "networks", "expose", "ports"):
                    if key in config:
                        entry[key] = config[key]
    services.setdefault(MAIN, {})
    return services


def declared_networks(documents: list[Any]) -> dict[str, dict[str, Any]]:
    networks: dict[str, dict[str, Any]] = {}
    for document in documents:
        if not isinstance(document, dict) or not isinstance(document.get("networks"), dict):
            continue
        for name, config in document["networks"].items():
            entry = networks.setdefault(str(name), {})
            if isinstance(config, dict):
                entry.update(config)
    return networks


def agent_namespace(services: dict[str, dict[str, Any]]) -> list[str]:
    """``main`` and the services whose namespace it shares, root last."""
    chain = [MAIN]
    while True:
        mode = services.get(chain[-1], {}).get("network_mode")
        if mode is None:
            return chain
        if not (isinstance(mode, str) and mode.startswith("service:")):
            raise ComposeRefused(
                f"service {chain[-1]!r} sets network_mode {mode!r}; the allowlist "
                "needs the agent's namespace behind the egress sidecar"
            )
        target = mode.removeprefix("service:")
        if target in chain or target not in services:
            raise ComposeRefused(f"service {chain[-1]!r} shares an unknown namespace {mode!r}")
        chain.append(target)


def _internal() -> dict[str, Any]:
    """An internal network: no route off the host, and, with the isolated
    gateway mode, no address on the host's side of the bridge."""
    return {
        "internal": True,
        "driver_opts": {"com.docker.network.bridge.gateway_mode_ipv4": "isolated"},
    }


def overlay(documents: list[Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    """The compose overlay, and what it did to each service.

    ``documents`` are the task's compose files in ``-f`` order. Refuses a
    task whose networking would give any service a route around the
    sidecar.
    """
    services = merged_services(documents)
    chain = agent_namespace(services)
    root = chain[-1]
    if "networks" in services[root]:
        raise ComposeRefused(
            f"service {root!r} holds the agent's namespace and declares its own "
            "networks; the allowlist needs it behind the egress sidecar"
        )
    others = [name for name in services if name not in chain and name != SIDECAR]
    for name in others:
        mode = services[name].get("network_mode")
        if mode is None or mode == "none":
            continue
        if isinstance(mode, str) and mode.startswith("service:"):
            if mode.removeprefix("service:") == SIDECAR:
                raise ComposeRefused(f"service {name!r} joins the egress sidecar itself")
            continue
        raise ComposeRefused(
            f"service {name!r} sets network_mode {mode!r}, which could reach the internet"
        )
    networks = declared_networks(documents)
    for name, config in networks.items():
        if config.get("external"):
            raise ComposeRefused(f"network {name!r} is external, so it can't be made internal")

    root_config: dict[str, Any] = {
        "network_mode": f"service:{SIDECAR}",
        "depends_on": {SIDECAR: {"condition": "service_healthy"}},
    }
    reset = [key for key in ("expose", "ports") if services[root].get(key)]
    for key in reset:
        root_config[key] = _Reset()
    body = {
        "services": {
            root: root_config,
            SIDECAR: {
                # Compose interpolates ``$``; ``$$`` passes one through.
                "entrypoint": ["/bin/sh", "-c", _WRAPPER.replace("$", "$$")],
                "networks": {
                    DEFAULT_NETWORK: {"aliases": list(chain)},
                    EGRESS_NETWORK: {},
                },
            },
        },
        "networks": {
            DEFAULT_NETWORK: _internal(),
            **{name: _internal() for name in networks if name != DEFAULT_NETWORK},
            EGRESS_NETWORK: {},
        },
    }
    summary = {
        "design": "tbench: agent namespace behind the sidecar, services on internal networks",
        "behind_sidecar": chain,
        "reset_on_behind_sidecar": {root: reset} if reset else {},
        "internal_services": others,
        "internal_networks": [DEFAULT_NETWORK, *(n for n in networks if n != DEFAULT_NETWORK)],
        "sidecar_aliases": list(chain),
    }
    return body, summary


def single_container_summary() -> dict[str, Any]:
    """The placement record for a task with no compose file of its own."""
    return {
        "design": "harbor: main alone behind the sidecar",
        "behind_sidecar": [MAIN],
        "reset_on_behind_sidecar": {},
        "internal_services": [],
        "internal_networks": [],
        "sidecar_aliases": [],
    }


def dump(body: dict[str, Any]) -> str:
    return yaml.dump(body, Dumper=_Dumper, sort_keys=False)
