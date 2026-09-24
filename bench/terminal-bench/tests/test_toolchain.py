"""Prebuilt toolchain layers: build once, reuse warm, place without network."""

import asyncio
import base64
import hashlib
import io
import json
import tarfile
import zipfile

import pytest

from tbench import toolchain
from tbench.coder_one import CoderOneDelegate
from tbench.results import setup_summary
from tbench.toolchain import (
    LayerSpec,
    ToolchainError,
    ensure_layer,
    layers_for,
    place_toolchain,
    platform_for,
)

SECRET = "sk-fixture-secret-value-0123456789"


def _tarball(files: dict[str, bytes], *, compression: str = "gz") -> bytes:
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode=f"w:{compression}") as archive:
        for name, data in files.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = 0o755
            archive.addfile(info, io.BytesIO(data))
    return buffer.getvalue()


PEM = b"-----BEGIN CERTIFICATE-----\nfixture\n-----END CERTIFICATE-----\n"


def _wheel(pem: bytes) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as wheel:
        wheel.writestr("certifi/cacert.pem", pem)
        wheel.writestr("certifi/__init__.py", "")
    return buffer.getvalue()


class _Registry:
    """Serves pinned-looking downloads and counts every fetch."""

    def __init__(self, monkeypatch, *, claude_payload: bytes = b"claude-binary"):
        self.calls: list[str] = []
        node = _tarball(
            {
                "node-v22.23.2-linux-x64/bin/node": b"#!/bin/sh\necho v22.23.2\n",
                "node-v22.23.2-linux-x64/lib/node_modules/npm/bin/npm-cli.js": b"",
            },
            compression="xz",
        )
        monkeypatch.setitem(
            toolchain.NODE_SHA256, "linux-x64", hashlib.sha256(node).hexdigest()
        )
        wheel = _wheel(PEM)
        monkeypatch.setitem(toolchain.CA_BUNDLE, "sha256", hashlib.sha256(wheel).hexdigest())
        monkeypatch.setitem(
            toolchain.CA_BUNDLE, "pem_sha256", hashlib.sha256(PEM).hexdigest()
        )
        codex = _tarball({"package/bin/codex.js": b"#!/usr/bin/env node\n"})
        native = _tarball(
            {"package/vendor/x86_64-unknown-linux-musl/bin/codex": b"\x7fELF"}
        )
        self.files = {
            toolchain.NODE_URL.format(version="22.23.2", platform="linux-x64"): node,
            "https://example.test/codex.tgz": codex,
            "https://example.test/codex-x64.tgz": native,
            f"{toolchain.NPM_REGISTRY}/@openai/codex/0.155.1": self._meta(
                "https://example.test/codex.tgz", codex
            ),
            f"{toolchain.NPM_REGISTRY}/@openai/codex/0.155.1-linux-x64": self._meta(
                "https://example.test/codex-x64.tgz", native
            ),
            f"{toolchain.CLAUDE_RELEASES}/2.1.280/manifest.json": json.dumps(
                {
                    "platforms": {
                        "linux-x64": {
                            "checksum": hashlib.sha256(claude_payload).hexdigest()
                        }
                    }
                }
            ).encode(),
            f"{toolchain.CLAUDE_RELEASES}/2.1.280/linux-x64/claude": claude_payload,
            toolchain.CA_BUNDLE["url"]: wheel,
        }

    @staticmethod
    def _meta(url: str, data: bytes) -> bytes:
        integrity = "sha512-" + base64.b64encode(hashlib.sha512(data).digest()).decode()
        return json.dumps({"dist": {"tarball": url, "integrity": integrity}}).encode()

    def __call__(self, url: str) -> bytes:
        self.calls.append(url)
        return self.files[url]


def test_platform_names_follow_the_architecture_and_c_library():
    assert platform_for("x86_64", False) == "linux-x64"
    assert platform_for("aarch64", True) == "linux-arm64-musl"
    with pytest.raises(ToolchainError):
        platform_for("riscv64", False)


def test_layers_put_node_first_and_pin_its_build():
    specs = layers_for("codex", "0.155.1", "linux-x64")
    assert [s.key for s in specs] == [
        "node-22.23.2-linux-x64",
        "ca-bundle-2026.7.22-linux-x64",
        "codex-0.155.1-linux-x64",
    ]
    assert layers_for("claude-code", "2.1.280", "linux-x64")[1].name == "claude-code"


def test_a_layer_builds_cold_once_then_reuses_warm(tmp_path, monkeypatch):
    fetch = _Registry(monkeypatch)
    spec = LayerSpec("codex", "0.155.1", "linux-x64")
    cold = ensure_layer(spec, root=tmp_path, fetch=fetch, credentials={})
    assert cold.cache == "cold"
    launcher = cold.root / "lib/node_modules/@openai/codex/bin/codex.js"
    native = (
        cold.root
        / "lib/node_modules/@openai/codex/node_modules/@openai/codex-linux-x64"
        / "vendor/x86_64-unknown-linux-musl/bin/codex"
    )
    assert launcher.is_file() and native.is_file()
    assert cold.manifest["links"] == {
        "codex": "lib/node_modules/@openai/codex/bin/codex.js"
    }
    fetched = len(fetch.calls)
    warm = ensure_layer(spec, root=tmp_path, fetch=fetch, credentials={})
    assert warm.cache == "warm"
    assert len(fetch.calls) == fetched
    assert warm.manifest["tree_sha256"] == cold.manifest["tree_sha256"]


def test_the_ca_bundle_layer_holds_the_pinned_bundle(tmp_path, monkeypatch):
    fetch = _Registry(monkeypatch)
    spec = LayerSpec("ca-bundle", "2026.7.22", "linux-x64")
    layer = ensure_layer(spec, root=tmp_path, fetch=fetch, credentials={})
    assert (layer.root / "cacert.pem").read_bytes() == PEM
    assert layer.links == {}
    assert layer.manifest["certificates"] == 1
    assert layer.manifest["source"]["member"] == "certifi/cacert.pem"
    assert fetch.calls == [toolchain.CA_BUNDLE["url"]]


def test_a_ca_bundle_that_differs_from_its_pin_is_refused(tmp_path, monkeypatch):
    fetch = _Registry(monkeypatch)
    fetch.files[toolchain.CA_BUNDLE["url"]] = _wheel(PEM + b"extra")
    with pytest.raises(ToolchainError, match="pinned sha256"):
        ensure_layer(
            LayerSpec("ca-bundle", "2026.7.22", "linux-x64"),
            root=tmp_path,
            fetch=fetch,
            credentials={},
        )
    with pytest.raises(ToolchainError, match="isn't pinned"):
        ensure_layer(
            LayerSpec("ca-bundle", "2020.1.1", "linux-x64"),
            root=tmp_path,
            fetch=fetch,
            credentials={},
        )


def test_a_changed_download_is_refused(tmp_path, monkeypatch):
    fetch = _Registry(monkeypatch)
    url = f"{toolchain.CLAUDE_RELEASES}/2.1.280/linux-x64/claude"
    fetch.files[url] = b"tampered"
    with pytest.raises(ToolchainError, match="checksum"):
        ensure_layer(
            LayerSpec("claude-code", "2.1.280", "linux-x64"),
            root=tmp_path,
            fetch=fetch,
            credentials={},
        )
    assert not (tmp_path / "claude-code-2.1.280-linux-x64").exists()


def test_a_layer_holding_a_credential_is_discarded(tmp_path, monkeypatch):
    fetch = _Registry(monkeypatch, claude_payload=SECRET.encode())
    with pytest.raises(ToolchainError) as caught:
        ensure_layer(
            LayerSpec("claude-code", "2.1.280", "linux-x64"),
            root=tmp_path,
            fetch=fetch,
            credentials={"env TEST_KEY": SECRET},
        )
    assert SECRET not in str(caught.value)
    assert "env TEST_KEY" in str(caught.value)
    assert not (tmp_path / "claude-code-2.1.280-linux-x64").exists()


def test_node_other_than_the_pinned_build_is_refused(tmp_path, monkeypatch):
    with pytest.raises(ToolchainError, match="isn't pinned"):
        ensure_layer(
            LayerSpec("node", "22.0.0", "linux-x64"),
            root=tmp_path,
            fetch=_Registry(monkeypatch),
            credentials={},
        )


class _Result:
    def __init__(self, stdout: str = "", return_code: int = 0):
        self.stdout = stdout
        self.stderr = ""
        self.return_code = return_code


class _Container:
    """Answers the platform probe and version checks; records copies."""

    def __init__(self, machine: str = "x86_64", libc: str = "glibc"):
        self.machine = machine
        self.libc = libc
        self.commands: list[str] = []
        self.uploads: list[tuple[str, str]] = []

    async def exec(self, command: str, **_: object) -> _Result:
        self.commands.append(command)
        if command.startswith("uname -m"):
            return _Result(f"{self.machine}\n{self.libc}\n")
        if command.endswith("node --version"):
            return _Result("v22.23.2\n")
        if command.endswith("codex --version"):
            return _Result("codex-cli 0.155.1\n")
        if command.endswith("claude --version"):
            return _Result("2.1.280 (Claude Code)\n")
        return _Result("ok")

    async def upload_dir(self, source: str, target: str) -> None:
        self.uploads.append((source, target))

    async def upload_file(self, source: str, target: str) -> None:
        self.uploads.append((source, target))


class _Agent:
    def __init__(self, container: _Container):
        self.container = container

    async def exec_as_root(self, environment, command: str, **_: object):
        environment.commands.append(command)
        return _Result("ok")


def test_placement_copies_links_and_verifies_without_network(tmp_path, monkeypatch):
    fetch = _Registry(monkeypatch)
    container = _Container()
    record = asyncio.run(
        place_toolchain(
            _Agent(container),
            container,
            "codex",
            "0.155.1",
            root=tmp_path,
            fetch=fetch,
            credentials={},
        )
    )
    assert record["mode"] == "prebuilt"
    assert record["cache"] == "cold"
    assert record["platform"] == "linux-x64"
    assert record["versions"] == {"node": "v22.23.2", "codex": "0.155.1"}
    assert [target for _, target in container.uploads] == [
        "/opt/openagents/toolchain/node-22.23.2-linux-x64",
        "/opt/openagents/toolchain/ca-bundle-2026.7.22-linux-x64",
        "/opt/openagents/toolchain/codex-0.155.1-linux-x64",
    ]
    links = next(c for c in container.commands if "ln -sf" in c)
    # Codex runs through a wrapper that starts the pinned Node by path.
    assert (
        "exec /opt/openagents/toolchain/node-22.23.2-linux-x64/bin/node "
        "/opt/openagents/toolchain/codex-0.155.1-linux-x64/"
        "lib/node_modules/@openai/codex/bin/codex.js" in links
    )
    # The wrapper gives Codex the pinned roots unless the caller chose its own.
    assert (
        '[ -n "$CODEX_CA_CERTIFICATE" ] || export CODEX_CA_CERTIFICATE='
        "/opt/openagents/toolchain/ca-bundle-2026.7.22-linux-x64/cacert.pem"
        in links
    )
    # The layer's Node links only where the image has no Node of its own.
    assert "[ -e /usr/local/bin/node ] || ln -sf" in links
    # Node's version check runs the layer's binary, not whatever is linked.
    assert (
        "/opt/openagents/toolchain/node-22.23.2-linux-x64/bin/node --version"
        in container.commands
    )
    # Nothing in the container reaches a package manager or the network.
    assert not any(
        word in command
        for command in container.commands
        for word in ("apt-get", "curl", "npm install", "nvm")
    )
    again = asyncio.run(
        place_toolchain(
            _Agent(container), container, "codex", "0.155.1",
            root=tmp_path, fetch=fetch, credentials={},
        )
    )
    assert again["cache"] == "warm"
    assert all(layer["build_ms"] == 0 for layer in again["layers"])


def test_placement_refuses_a_musl_codex_so_the_caller_can_fall_back(
    tmp_path, monkeypatch
):
    container = _Container(libc="musl")
    with pytest.raises(ToolchainError):
        asyncio.run(
            place_toolchain(
                _Agent(container), container, "codex", "0.155.1",
                root=tmp_path, fetch=_Registry(monkeypatch), credentials={},
            )
        )


def _binary(tmp_path) -> tuple[str, str]:
    path = tmp_path / "coder-one"
    path.write_bytes(b"#!/bin/sh\n")
    return str(path), hashlib.sha256(b"#!/bin/sh\n").hexdigest()


def _delegate(tmp_path, **kwargs) -> CoderOneDelegate:
    path, digest = _binary(tmp_path)
    return CoderOneDelegate(
        logs_dir=tmp_path, artifact_path=path, artifact_sha256=digest, **kwargs
    )


def test_delegate_prebuilt_install_writes_its_setup_record(tmp_path, monkeypatch):
    agent = _delegate(tmp_path, delegate="always")

    async def placed(agent, environment, executor, version, **_):
        assert (executor, version) == ("claude-code", "2.1.280")
        return toolchain.setup_record(
            mode="prebuilt", platform="linux-x64", layers=[],
            phases_ms={"copy": 900}, versions={"claude-code": version},
        )

    async def refuse(*_: object, **__: object) -> None:
        raise AssertionError("the prebuilt path ran a network install")

    monkeypatch.setattr("tbench.coder_one.place_toolchain", placed)
    monkeypatch.setattr(agent, "ensure_system_dependencies", refuse)
    monkeypatch.setattr(agent, "exec_as_agent", refuse)

    class _Probe(_Container):
        async def exec(self, command: str, **kw: object) -> _Result:
            if "readlink" in command:
                return _Result(
                    "/opt/openagents/toolchain/claude-code-2.1.280-linux-x64/bin/claude\n"
                    "2.1.279 (Claude Code)\n"
                )
            return await super().exec(command, **kw)

    with pytest.raises(Exception, match="2.1.279 installed"):
        asyncio.run(agent.install(_Probe()))
    record = json.loads((tmp_path / "toolchain-setup.json").read_text())
    assert record["mode"] == "prebuilt"
    assert record["install_ms"] == 900


def test_delegate_falls_back_to_a_guarded_network_install(tmp_path, monkeypatch):
    agent = _delegate(tmp_path, delegate="always")
    ran: list[str] = []

    async def unavailable(*_: object, **__: object):
        raise ToolchainError("no layer for linux-riscv64")

    async def network(environment) -> None:
        ran.append("network")

    monkeypatch.setattr("tbench.coder_one.place_toolchain", unavailable)
    monkeypatch.setattr(agent, "_install_claude_network", network)
    with pytest.raises(Exception):
        asyncio.run(agent.install(_Container()))
    assert ran == ["network"]
    record = json.loads((tmp_path / "toolchain-setup.json").read_text())
    assert record["mode"] == "network"
    assert record["cache"] == "none"
    assert "linux-riscv64" in record["note"]
    assert set(record["phases_ms"]) == {"guard_wait", "network_install"}


def test_the_network_codex_install_pins_node(tmp_path):
    command = _delegate(
        tmp_path, delegate="always", delegate_agent="codex", toolchain="network"
    ).codex_install_command()
    assert "nvm install 22.23.2 && nvm alias default 22.23.2" in command
    assert "nvm install 22 " not in command


def test_an_unknown_toolchain_mode_is_refused(tmp_path):
    with pytest.raises(Exception, match="prebuilt or network"):
        _delegate(tmp_path, delegate="always", toolchain="image")


def test_setup_summary_keeps_cache_state_and_failures(tmp_path):
    (tmp_path / "agent").mkdir()
    timing = {"agent_setup_ms": 12000}
    unknown = setup_summary(tmp_path, timing, "completed")
    assert (unknown["mode"], unknown["cache"], unknown["failed"]) == (
        "unknown",
        "unknown",
        False,
    )
    (tmp_path / "agent" / "toolchain-setup.json").write_text(
        json.dumps(
            {
                "mode": "prebuilt",
                "cache": "warm",
                "install_ms": 4200,
                "layers": [{"key": "node-22.23.2-linux-x64"}],
            }
        )
    )
    summary = setup_summary(tmp_path, timing, "install_failure")
    assert summary["cache"] == "warm"
    assert summary["install_ms"] == 4200
    assert summary["agent_setup_ms"] == 12000
    assert summary["failed"] is True
    assert summary["layers"] == ["node-22.23.2-linux-x64"]
    assert "agent_setup_ms" in summary["boundaries"]
