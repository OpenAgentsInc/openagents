"""The Terminal-Bench 4.0 catalog, its job profile, and the GPU environment."""

import json
from pathlib import Path
from types import SimpleNamespace

import pytest

from tbench import host, paths
from tbench.agents import load_agents
from tbench.catalog import base_image, entries_from_checkout
from tbench.gpu_docker import CdiDockerEnvironment, add_gpu_device, nvidia_cdi_spec
from tbench.jobconfig import build_job_config, load_job_profile
from tbench.panel import load_panel
from tbench.runner import RunRefused, RunRequest, materialize

GPU_TASKS = {"fp8-rmsnorm-gemm", "jax-speedrun-gpu", "math-eval-grader"}


@pytest.fixture(scope="module")
def tb4():
    return load_panel(catalog="tb4")


def test_tb4_pins_v4_with_all_66_tasks(tb4):
    assert tb4.git_commit_id == "452bf305c6daa62fc59061d22133a7cbc7c1572e"
    assert tb4.ref == "v4.0.0"
    assert tb4.checkout() == paths.upstream_checkout("terminal-bench-v4.0.0")
    assert len(tb4.tasks) == 66
    assert all(task.agent_timeout_sec == 28800 for task in tb4.tasks)
    cpus = sorted(task.resources.cpus for task in tb4.tasks)
    assert cpus.count(2) == 44 and cpus.count(4) == 14
    assert cpus.count(8) == 6 and cpus.count(16) == 2
    assert {t.id for t in tb4.tasks if t.requires_gpu_runtime} == GPU_TASKS


def test_the_panel_pin_is_untouched(tb4):
    panel = load_panel()
    assert panel.git_commit_id.startswith("3b5caaa")
    assert panel.checkout() == paths.upstream_checkout()
    # Same task name, another commit: each catalog resolves its own.
    assert panel.task("batched-eval-parity").path == tb4.task("batched-eval-parity").path
    assert panel.task("math-eval-grader").excluded
    assert not tb4.task("math-eval-grader").excluded


def test_peak_resources_cover_a_separate_verifier(tb4):
    task = tb4.task("batched-eval-parity")
    assert task.verifier_resources is not None
    peak = task.peak_resources
    assert peak.cpus >= task.resources.cpus
    assert peak.memory_mb >= task.verifier_resources.memory_mb


def test_the_tb4_job_profile_draws_from_the_catalog(tb4):
    profile = load_job_profile("tb4")
    assert profile.catalog == "tb4"
    assert set(profile.task_ids) == {task.id for task in tb4.tasks}
    assert profile.environment["import_path"] == "tbench.gpu_docker:CdiDockerEnvironment"
    config = build_job_config(
        tb4,
        profile,
        tb4.select(["cad-model"]),
        load_agents()["oracle"],
        checkout=Path("/checkout"),
    )
    assert config["tasks"] == [{"path": "/checkout/tasks/cad-model"}]
    assert config["environment"]["import_path"].endswith("CdiDockerEnvironment")


def test_the_checked_catalog_matches_its_checkout(tb4):
    checkout = tb4.checkout()
    if not (checkout / "tasks").is_dir():
        pytest.skip("no TB4 checkout; `tbench tasks checkout` creates it")
    data = json.loads((paths.PROFILES_DIR / "tasks.json").read_text())
    checked = {entry["id"]: entry for entry in data["catalogs"]["tb4"]["tasks"]}
    for entry in entries_from_checkout(checkout, profiles=["tb4"]):
        mine = {k: v for k, v in checked[entry["id"]].items() if k in entry}
        assert mine == entry, entry["id"]


def test_base_image_is_the_final_stage(tmp_path):
    dockerfile = tmp_path / "Dockerfile"
    dockerfile.write_text(
        "FROM python:3.12 AS builder\nRUN python - <<EOF\nfrom pathlib import Path\nEOF\n"
        "FROM --platform=linux/amd64 debian:12-slim\n"
    )
    assert base_image(dockerfile) == "debian:12-slim"


def test_a_gpu_task_is_refused_and_recorded_without_a_gpu(tmp_path, tb4, monkeypatch):
    monkeypatch.setenv("TBENCH_CDI_DIRS", str(tmp_path / "no-cdi"))
    monkeypatch.setattr(host, "docker_info", lambda: {"Runtimes": {"runc": {}}})
    request = RunRequest(
        panel=tb4,
        profile=load_job_profile("tb4"),
        agent=load_agents()["oracle"],
        tasks=tb4.select(["math-eval-grader"]),
        jobs_dir=tmp_path,
        job_name="tb4--oracle--math-eval-grader",
    )
    with pytest.raises(RunRefused, match="needs a GPU"):
        materialize(request, record_refusal=True)
    refusals = list((tmp_path / request.job_name / "tbench" / "refusals").glob("*.json"))
    record = json.loads(refusals[0].read_text())
    assert record["stage"] == "gpu_runtime"


def _cdi(tmp_path):
    directory = tmp_path / "cdi"
    directory.mkdir()
    (directory / "nvidia-container-toolkit.json").write_text(
        json.dumps({"cdiVersion": "0.5.0", "kind": "nvidia.com/gpu", "devices": []})
    )
    return directory


def test_a_cdi_spec_makes_a_gpu_available(tmp_path, monkeypatch):
    assert nvidia_cdi_spec([tmp_path / "missing"]) is None
    directory = _cdi(tmp_path)
    assert nvidia_cdi_spec([directory]).name == "nvidia-container-toolkit.json"
    monkeypatch.setenv("TBENCH_CDI_DIRS", str(directory))
    assert host.gpu_refusal("math-eval-grader", {"Runtimes": {"runc": {}}}) is None
    monkeypatch.setenv("TBENCH_CDI_DIRS", str(tmp_path / "missing"))
    assert "NVIDIA" in host.gpu_refusal("math-eval-grader", {"Runtimes": {}})
    assert host.gpu_refusal("x", {"Runtimes": {"nvidia": {}}}) is None


def test_add_gpu_device_extends_the_resource_override(tmp_path):
    path = tmp_path / "resources.json"
    path.write_text(json.dumps({"services": {"main": {"cpus": 4.0}}}))
    add_gpu_device(path)
    add_gpu_device(path)
    main = json.loads(path.read_text())["services"]["main"]
    assert main == {"cpus": 4.0, "devices": ["nvidia.com/gpu=all"]}


def _environment(gpus: int):
    environment = object.__new__(CdiDockerEnvironment)
    environment.task_env_config = SimpleNamespace(gpus=gpus)
    environment._enable_egress_control = False
    return environment


def test_the_cdi_environment_adds_the_device_only_for_gpu_tasks(tmp_path, monkeypatch):
    from harbor.environments.docker.docker import DockerEnvironment

    rendered = tmp_path / "render.json"

    def stock(self):
        rendered.write_text(json.dumps({"services": {"main": {"mem_limit": "4096m"}}}))
        return rendered

    monkeypatch.setattr(DockerEnvironment, "_write_resources_compose_file", stock)
    _environment(0)._write_resources_compose_file()
    assert "devices" not in json.loads(rendered.read_text())["services"]["main"]
    _environment(1)._write_resources_compose_file()
    assert json.loads(rendered.read_text())["services"]["main"]["devices"] == [
        "nvidia.com/gpu=all"
    ]


def test_the_cdi_environment_declares_gpus_only_with_a_spec(tmp_path, monkeypatch):
    monkeypatch.setenv("TBENCH_CDI_DIRS", str(tmp_path / "missing"))
    assert _environment(1).capabilities.gpus is False
    monkeypatch.setenv("TBENCH_CDI_DIRS", str(_cdi(tmp_path)))
    assert _environment(1).capabilities.gpus is True
