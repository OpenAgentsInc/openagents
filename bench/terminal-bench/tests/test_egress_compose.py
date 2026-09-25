"""The compose overlay that holds a multi-service task's agent behind the
egress sidecar while its services stay on internal networks (#9607)."""

import json
from pathlib import Path
from types import SimpleNamespace

import pytest
import yaml
from harbor.environments.docker.docker import DockerEnvironment

from tbench import egress_compose, netpolicy
from tbench.egress_compose import SIDECAR, ComposeRefused, overlay


class _Loader(yaml.SafeLoader):
    pass


_Loader.add_constructor("!reset", lambda loader, node: ("!reset", loader.construct_sequence(node)))


def _roundtrip(body):
    return yaml.load(egress_compose.dump(body), Loader=_Loader)


@pytest.fixture(autouse=True)
def _restore_harbor():
    yield
    netpolicy.uninstall()


def _task(services, networks=None):
    document = {"services": services}
    if networks is not None:
        document["networks"] = networks
    return [document]


def test_services_with_expose_stay_off_the_sidecar_on_an_internal_network():
    body, summary = overlay(
        _task(
            {
                "main": {"depends_on": {"kafka": {"condition": "service_healthy"}}},
                "kafka": {"image": "kafka", "expose": ["9092"]},
                "customer": {"build": {"context": "./customer"}, "expose": ["9000"]},
            }
        )
    )
    services = _roundtrip(body)["services"]
    # Only the agent's container shares the sidecar's namespace.
    assert services["main"]["network_mode"] == f"service:{SIDECAR}"
    assert services["main"]["depends_on"] == {SIDECAR: {"condition": "service_healthy"}}
    assert set(services) == {"main", SIDECAR}
    # The sidecar carries main's name on the task network and has its own way out.
    assert body["services"][SIDECAR]["networks"] == {
        "default": {"aliases": ["main"]},
        "tbench-egress": {},
    }
    assert body["networks"]["default"]["internal"] is True
    assert body["networks"]["default"]["driver_opts"] == {
        "com.docker.network.bridge.gateway_mode_ipv4": "isolated"
    }
    assert body["networks"]["tbench-egress"] == {}
    assert summary["behind_sidecar"] == ["main"]
    assert summary["internal_services"] == ["kafka", "customer"]
    assert summary["reset_on_behind_sidecar"] == {}


def test_services_with_ports_stay_off_the_sidecar_and_keep_their_ports():
    body, summary = overlay(
        _task({"main": {}, "api": {"image": "api", "ports": ["8080:8080"]}})
    )
    assert "api" not in body["services"]
    assert summary["internal_services"] == ["api"]


def test_services_without_expose_or_ports_are_treated_the_same():
    body, summary = overlay(_task({"main": {}, "seeder": {"build": {"context": "./seeder"}}}))
    assert set(body["services"]) == {"main", SIDECAR}
    assert summary["internal_services"] == ["seeder"]


def test_a_main_that_shares_a_service_namespace_puts_that_service_behind_the_sidecar():
    body, summary = overlay(
        _task(
            {
                "main": {"network_mode": "service:warehouse-api"},
                "warehouse-api": {"expose": ["4100", "4101"], "ports": ["4100:4100"]},
            }
        )
    )
    services = _roundtrip(body)["services"]
    assert "main" not in services
    root = services["warehouse-api"]
    assert root["network_mode"] == f"service:{SIDECAR}"
    # Docker refuses a port on a container-mode service, so both are reset.
    assert root["expose"] == ("!reset", [])
    assert root["ports"] == ("!reset", [])
    assert services[SIDECAR]["networks"]["default"] == {"aliases": ["main", "warehouse-api"]}
    assert summary["behind_sidecar"] == ["main", "warehouse-api"]
    assert summary["reset_on_behind_sidecar"] == {"warehouse-api": ["expose", "ports"]}


def test_task_networks_become_internal_too():
    body, summary = overlay(
        _task(
            {"main": {}, "db": {"networks": ["backend"]}},
            networks={"backend": {"driver": "bridge"}},
        )
    )
    assert body["networks"]["backend"]["internal"] is True
    assert summary["internal_networks"] == ["default", "backend"]


@pytest.mark.parametrize(
    ("services", "networks", "message"),
    [
        ({"main": {"networks": ["x"]}}, None, "declares its own networks"),
        ({"main": {"network_mode": "host"}}, None, "network_mode 'host'"),
        ({"main": {}, "proxy": {"network_mode": "host"}}, None, "could reach the internet"),
        ({"main": {}, "proxy": {"network_mode": "bridge"}}, None, "could reach the internet"),
        ({"main": {"network_mode": "service:gone"}}, None, "unknown namespace"),
        ({"main": {}}, {"shared": {"external": True}}, "external"),
    ],
)
def test_networking_with_a_route_around_the_sidecar_is_refused(services, networks, message):
    with pytest.raises(ComposeRefused, match=message):
        overlay(_task(services, networks))


def test_the_sidecar_wrapper_escapes_compose_interpolation():
    body, _ = overlay(_task({"main": {}, "db": {}}))
    command = body["services"][SIDECAR]["entrypoint"][2]
    assert "$$(ip -4 route show default" in command
    assert "$(" not in command.replace("$$", "")
    assert command.rstrip().endswith("exec /opt/egress-sidecar/entrypoint.sh")


def _env(tmp_path: Path, compose: dict | None):
    environment_dir = tmp_path / "environment"
    environment_dir.mkdir()
    if compose is not None:
        (environment_dir / "docker-compose.yaml").write_text(yaml.safe_dump(compose))
    trial_dir = tmp_path / "trial"
    trial_dir.mkdir()
    env = SimpleNamespace(
        _enable_egress_control=True,
        _environment_docker_compose_path=environment_dir / "docker-compose.yaml",
        extra_docker_compose_paths=[],
        trial_paths=SimpleNamespace(trial_dir=trial_dir),
        _egress_control_services_compose_temp_dir=None,
        _egress_control_services_compose_path=None,
        _EGRESS_CONTROL_SERVICE_NAME=SIDECAR,
    )
    env._cleanup_egress_control_services_compose_file = lambda: None
    env._egress_controlled_service_names = lambda: DockerEnvironment._egress_controlled_service_names(env)
    return env, trial_dir


def test_the_allowlist_plugin_replaces_harbors_overlay_and_records_the_placement(tmp_path):
    netpolicy.install("allowlist")
    env, trial_dir = _env(
        tmp_path, {"services": {"main": {}, "kafka": {"expose": ["9092"]}}}
    )
    path = DockerEnvironment._write_egress_control_services_compose_file(env)
    written = yaml.load(path.read_text(), Loader=_Loader)
    assert "kafka" not in written["services"]
    assert written["networks"]["default"]["internal"] is True
    record = json.loads((trial_dir / netpolicy.RECORD_NAME).read_text())
    assert record["services"]["internal_services"] == ["kafka"]
    # A later phase record keeps the placement.
    netpolicy._write(trial_dir, {"schema": netpolicy.RECORD_SCHEMA, "agent_phase_public": False})
    assert netpolicy.trial_network(trial_dir)["services"]["behind_sidecar"] == ["main"]


def test_a_single_container_task_keeps_harbors_overlay(tmp_path):
    netpolicy.install("allowlist")
    env, trial_dir = _env(tmp_path, None)
    path = DockerEnvironment._write_egress_control_services_compose_file(env)
    assert json.loads(path.read_text()) == {
        "services": {
            "main": {
                "network_mode": f"service:{SIDECAR}",
                "depends_on": {SIDECAR: {"condition": "service_healthy"}},
            }
        }
    }
    record = json.loads((trial_dir / netpolicy.RECORD_NAME).read_text())
    assert record["services"]["behind_sidecar"] == ["main"]
    assert record["services"]["internal_services"] == []


def test_the_harbor_mode_keeps_harbors_overlay(tmp_path):
    netpolicy.install("harbor")
    env, _ = _env(tmp_path, {"services": {"main": {}, "kafka": {"expose": ["9092"]}}})
    path = DockerEnvironment._write_egress_control_services_compose_file(env)
    assert json.loads(path.read_text())["services"]["kafka"]["network_mode"] == f"service:{SIDECAR}"
