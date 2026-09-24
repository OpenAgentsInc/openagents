"""Prebuilt agent toolchain layers: install once on the host, copy per trial.

A network install of the delegate CLIs costs minutes per trial:
``apt-get install nodejs npm``, nvm, ``nvm install 22``, then ``npm
install -g`` or Claude Code's bootstrap download. Eight concurrent trials
pay it eight times and time out. A layer is the finished result of one of
those installs, built once on the host from pinned, digest-checked public
downloads and cached under ``<state>/toolchains/<key>/``. Each trial copies
the layer into ``/opt/openagents/toolchain/<key>/`` and links its
executables into ``/usr/local/bin``: no package manager, no network, and
no image rebuild.

A layer's key is its name, version, and platform
(``linux-x64``, ``linux-arm64``, or a ``-musl`` variant). The layers are
self-contained binaries with no dependency on the task image beyond the
C library, so the platform is the whole image identity they need. Layers
hold public release files only: no credential, no task state. The build
scans each layer for the host's credential values anyway, and refuses a
layer that holds one.

Codex also gets a CA bundle layer. Codex verifies TLS against the image's
root certificates, and some task images, such as ``bun-sourcemap-leak``,
have none: every connection then fails with ``UnknownIssuer`` and Codex
retries until its deadline (issue #9581). Claude Code bundles its own
roots, so it needs no such layer.
"""

from __future__ import annotations

import base64
import contextlib
import fcntl
import hashlib
import io
import json
import shutil
import tarfile
import tempfile
import time
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any, Callable

from . import paths

LAYER_SCHEMA = "openagents.tbench.toolchain-layer.v1"
SETUP_SCHEMA = "openagents.tbench.toolchain-setup.v1"
CONTAINER_ROOT = PurePosixPath("/opt/openagents/toolchain")
LINK_DIR = PurePosixPath("/usr/local/bin")

# The Node build `nvm install 22` resolved on 2026-09-22, pinned with the
# digests from https://nodejs.org/dist/v22.23.2/SHASUMS256.txt.
NODE_VERSION = "22.23.2"
NODE_SHA256 = {
    "linux-x64": "d60acfe00a2932254bb0ad20e01b0d74397a0875595de719654b214f4b03f307",
    "linux-arm64": "fff4078c5def658577f92c88db7db3bc0072924bfb93fe52c1e744a54e94abb8",
}
NODE_URL = "https://nodejs.org/dist/v{version}/node-v{version}-{platform}.tar.xz"
NPM_REGISTRY = "https://registry.npmjs.org"
CLAUDE_RELEASES = "https://downloads.claude.ai/claude-code-releases"

# Mozilla's root certificates as the certifi 2026.7.22 wheel packages them,
# pinned with the wheel's sha256 from PyPI and the extracted bundle's own
# sha256, so every build yields the same bytes. The file's path in PyPI's
# storage is content-addressed.
CA_BUNDLE = {
    "version": "2026.7.22",
    "url": (
        "https://files.pythonhosted.org/packages/0b/a7/"
        "71ac2cff56fec219ed242bb11b8efb69fcc4bec75db06fb7bfe35de520e6/"
        "certifi-2026.7.22-py3-none-any.whl"
    ),
    "sha256": "62f22742b58a1a33014a2b6b706588a8d7e2a88ae7bd1a6ebe8c992928483775",
    "member": "certifi/cacert.pem",
    "pem_sha256": "9cc2a774b5198dcff14d9be1e66091f538975d867ce029a96bce15a55dfd730f",
}
CA_BUNDLE_FILE = "cacert.pem"
# Codex's own variable for extra roots. It reaches Codex alone, where
# ``SSL_CERT_FILE`` would also hand the task's own tools a CA store the
# image doesn't have.
CODEX_CA_VARIABLE = "CODEX_CA_CERTIFICATE"

Fetch = Callable[[str], bytes]


class ToolchainError(RuntimeError):
    """A layer that can't be built or placed as pinned."""


@dataclass(frozen=True)
class LayerSpec:
    """One layer's identity: the executor, its pinned version, a platform."""

    name: str
    version: str
    platform: str

    @property
    def key(self) -> str:
        return f"{self.name}-{self.version}-{self.platform}"

    @property
    def container_dir(self) -> PurePosixPath:
        return CONTAINER_ROOT / self.key


@dataclass
class Layer:
    """A built layer on the host and whether this call built it."""

    spec: LayerSpec
    root: Path
    manifest: dict[str, Any]
    cache: str  # "warm" when reused, "cold" when this call built it
    build_ms: int

    @property
    def links(self) -> dict[str, str]:
        return self.manifest["links"]


def platform_for(machine: str, musl: bool) -> str:
    """The layer platform for ``uname -m`` and the C library."""
    arch = {"x86_64": "x64", "amd64": "x64", "aarch64": "arm64", "arm64": "arm64"}
    if machine.strip() not in arch:
        raise ToolchainError(f"no toolchain layers for architecture {machine!r}")
    base = f"linux-{arch[machine.strip()]}"
    return f"{base}-musl" if musl else base


def cache_root() -> Path:
    return paths.state_dir() / "toolchains"


def _fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=300) as response:  # noqa: S310
        return response.read()


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _extract(data: bytes, dest: Path, *, strip: str) -> None:
    """Unpack a tarball into ``dest``, dropping the leading ``strip`` dir."""
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        members = []
        for member in archive.getmembers():
            parts = PurePosixPath(member.name).parts
            if not parts or parts[0] != strip:
                continue
            member.name = str(PurePosixPath(*parts[1:])) if len(parts) > 1 else "."
            members.append(member)
        archive.extractall(dest, members=members, filter="data")


def _build_node(spec: LayerSpec, root: Path, fetch: Fetch) -> dict[str, Any]:
    if spec.platform not in NODE_SHA256 or spec.version != NODE_VERSION:
        raise ToolchainError(
            f"Node {spec.version} for {spec.platform} isn't pinned; "
            f"the pinned build is {NODE_VERSION} for "
            f"{', '.join(sorted(NODE_SHA256))}"
        )
    url = NODE_URL.format(version=spec.version, platform=spec.platform)
    data = fetch(url)
    if _sha256(data) != NODE_SHA256[spec.platform]:
        raise ToolchainError(f"{url} doesn't match its pinned sha256")
    _extract(data, root, strip=f"node-v{spec.version}-{spec.platform}")
    return {
        "source": {"url": url, "sha256": NODE_SHA256[spec.platform]},
        "links": {"node": "bin/node", "npm": "bin/npm", "npx": "bin/npx"},
        "version_command": "node --version",
        "expected_version": f"v{spec.version}",
    }


def _npm_tarball(name: str, version: str, fetch: Fetch) -> tuple[str, bytes, str]:
    meta = json.loads(fetch(f"{NPM_REGISTRY}/{name}/{version}"))
    dist = meta["dist"]
    data = fetch(dist["tarball"])
    algorithm, _, expected = dist["integrity"].partition("-")
    if algorithm != "sha512":
        raise ToolchainError(f"{name}@{version}: unsupported integrity {algorithm}")
    actual = base64.b64encode(hashlib.sha512(data).digest()).decode()
    if actual != expected:
        raise ToolchainError(f"{name}@{version}: tarball fails its registry integrity")
    return dist["tarball"], data, dist["integrity"]


def _build_codex(spec: LayerSpec, root: Path, fetch: Fetch) -> dict[str, Any]:
    if spec.platform.endswith("-musl"):
        raise ToolchainError("the Codex layer runs on the Node layer, which is glibc only")
    arch = spec.platform.removeprefix("linux-")
    package = root / "lib" / "node_modules" / "@openai" / "codex"
    main_url, main, main_integrity = _npm_tarball("@openai/codex", spec.version, fetch)
    _extract(main, package, strip="package")
    native_url, native, native_integrity = _npm_tarball(
        "@openai/codex", f"{spec.version}-linux-{arch}", fetch
    )
    # Where `npm install -g` puts the optional platform package, so the
    # launcher's require.resolve finds it.
    _extract(
        native,
        package / "node_modules" / "@openai" / f"codex-linux-{arch}",
        strip="package",
    )
    return {
        "source": {
            "packages": [
                {"url": main_url, "integrity": main_integrity},
                {"url": native_url, "integrity": native_integrity},
            ]
        },
        "links": {"codex": "lib/node_modules/@openai/codex/bin/codex.js"},
        "requires": ["node"],
        "version_command": "codex --version",
        "expected_version": spec.version,
    }


def _build_ca_bundle(spec: LayerSpec, root: Path, fetch: Fetch) -> dict[str, Any]:
    import zipfile

    if spec.version != CA_BUNDLE["version"]:
        raise ToolchainError(
            f"CA bundle {spec.version} isn't pinned; the pinned bundle is "
            f"{CA_BUNDLE['version']}"
        )
    data = fetch(CA_BUNDLE["url"])
    if _sha256(data) != CA_BUNDLE["sha256"]:
        raise ToolchainError(f"{CA_BUNDLE['url']} doesn't match its pinned sha256")
    with zipfile.ZipFile(io.BytesIO(data)) as wheel:
        pem = wheel.read(CA_BUNDLE["member"])
    if _sha256(pem) != CA_BUNDLE["pem_sha256"]:
        raise ToolchainError(f"{CA_BUNDLE['member']} doesn't match its pinned sha256")
    (root / CA_BUNDLE_FILE).write_bytes(pem)
    return {
        "source": {
            "url": CA_BUNDLE["url"],
            "sha256": CA_BUNDLE["sha256"],
            "member": CA_BUNDLE["member"],
            "member_sha256": CA_BUNDLE["pem_sha256"],
        },
        "links": {},
        "certificates": pem.count(b"-----BEGIN CERTIFICATE-----"),
    }


def _build_claude(spec: LayerSpec, root: Path, fetch: Fetch) -> dict[str, Any]:
    base = f"{CLAUDE_RELEASES}/{spec.version}"
    manifest = json.loads(fetch(f"{base}/manifest.json"))
    entry = (manifest.get("platforms") or {}).get(spec.platform)
    if not entry or not entry.get("checksum"):
        raise ToolchainError(
            f"Claude Code {spec.version} has no {spec.platform} build"
        )
    url = f"{base}/{spec.platform}/claude"
    data = fetch(url)
    if _sha256(data) != entry["checksum"]:
        raise ToolchainError(f"{url} doesn't match its release manifest checksum")
    binary = root / "bin" / "claude"
    binary.parent.mkdir(parents=True, exist_ok=True)
    binary.write_bytes(data)
    binary.chmod(0o755)
    return {
        "source": {"url": url, "sha256": entry["checksum"]},
        "links": {"claude": "bin/claude"},
        "version_command": "claude --version",
        "expected_version": spec.version,
    }


BUILDERS: dict[str, Callable[[LayerSpec, Path, Fetch], dict[str, Any]]] = {
    "node": _build_node,
    "codex": _build_codex,
    "claude-code": _build_claude,
    "ca-bundle": _build_ca_bundle,
}


def tree_digest(root: Path) -> tuple[str, int, int]:
    """A digest over every path, link target, and file byte under ``root``."""
    digest = hashlib.sha256()
    files = 0
    size = 0
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            digest.update(f"L {relative} {path.readlink()}\n".encode())
        elif path.is_file():
            data = path.read_bytes()
            files += 1
            size += len(data)
            digest.update(f"F {relative} {_sha256(data)}\n".encode())
    return digest.hexdigest(), files, size


def _scan(root: Path, credentials: dict[str, str]) -> list[str]:
    needles = [(name, value.encode()) for name, value in credentials.items()]
    found = []
    for path in root.rglob("*"):
        if path.is_file() and not path.is_symlink():
            data = path.read_bytes()
            found.extend(name for name, needle in needles if needle in data)
    return sorted(set(found))


@contextlib.contextmanager
def _locked(path: Path):
    """One builder per layer across threads and processes."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(handle, fcntl.LOCK_UN)


def ensure_layer(
    spec: LayerSpec,
    *,
    root: Path | None = None,
    fetch: Fetch = _fetch,
    credentials: dict[str, str] | None = None,
) -> Layer:
    """Reuse the cached layer, or build it once and cache it."""
    if spec.name not in BUILDERS:
        raise ToolchainError(f"no layer builder for {spec.name!r}")
    base = (root or cache_root()) / spec.key
    manifest_path = base / "layer.json"
    started = time.monotonic()
    with _locked(base.parent / f"{spec.key}.lock"):
        if manifest_path.is_file():
            manifest = json.loads(manifest_path.read_text())
            return Layer(spec, base / "root", manifest, "warm", int((time.monotonic() - started) * 1000))
        if credentials is None:
            from .retain import known_credentials

            credentials = known_credentials()
        stage = Path(tempfile.mkdtemp(dir=base.parent, prefix=f".{spec.key}-"))
        try:
            layer_root = stage / "root"
            layer_root.mkdir()
            details = BUILDERS[spec.name](spec, layer_root, fetch)
            leaked = _scan(layer_root, credentials)
            if leaked:
                raise ToolchainError(
                    f"{spec.key} holds credential values ({', '.join(leaked)}); "
                    "the layer was discarded"
                )
            tree, files, size = tree_digest(layer_root)
            build_ms = int((time.monotonic() - started) * 1000)
            manifest = {
                "schema": LAYER_SCHEMA,
                "name": spec.name,
                "version": spec.version,
                "platform": spec.platform,
                "key": spec.key,
                "tree_sha256": tree,
                "files": files,
                "bytes": size,
                "built_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
                "build_ms": build_ms,
                "credential_scan": {
                    "credentials_checked": len(credentials),
                    "matches": 0,
                },
                **details,
            }
            (stage / "layer.json").write_text(json.dumps(manifest, indent=2) + "\n")
            if base.exists():
                shutil.rmtree(base)
            stage.rename(base)
        except BaseException:
            shutil.rmtree(stage, ignore_errors=True)
            raise
        return Layer(spec, base / "root", manifest, "cold", build_ms)


def layers_for(executor: str, version: str, platform: str) -> list[LayerSpec]:
    """The layers one executor needs, Node first."""
    node = LayerSpec("node", NODE_VERSION, platform)
    if executor == "codex":
        return [
            node,
            LayerSpec("ca-bundle", CA_BUNDLE["version"], platform),
            LayerSpec("codex", version, platform),
        ]
    if executor == "claude-code":
        # Claude Code is one native binary; Node rides along because the
        # network install put Node on the PATH, and a delegate may use it.
        return [node, LayerSpec("claude-code", version, platform)]
    raise ToolchainError(f"no toolchain for executor {executor!r}")


# Executables a task image may already ship. A task built on `node:20`
# keeps its own Node: the layer's Node is linked only where the image has
# none, and anything that needs the pinned Node runs it by absolute path.
KEEP_IMAGE_LINKS = frozenset({"node", "npm", "npx"})


def _node_binary(layers: list[Layer]) -> PurePosixPath | None:
    for layer in layers:
        if layer.spec.name == "node":
            return layer.spec.container_dir / layer.links["node"]
    return None


def _ca_bundle(layers: list[Layer]) -> PurePosixPath | None:
    for layer in layers:
        if layer.spec.name == "ca-bundle":
            return layer.spec.container_dir / CA_BUNDLE_FILE
    return None


def link_command(layers: list[Layer]) -> str:
    """The root command that puts every layer's executables on the PATH.

    Node, npm, and npx link into ``/usr/local/bin`` only when the image has
    none of its own there, so a task's own Node version survives. A layer
    that runs on Node (Codex) gets a small wrapper that starts the pinned
    Node by absolute path, so it works whatever Node the image carries.
    With a CA bundle layer, the Codex wrapper also points
    ``CODEX_CA_CERTIFICATE`` at the bundle unless the caller set it, so
    Codex has the same roots in every image.
    """
    parts = [f"mkdir -p {LINK_DIR}"]
    node = _node_binary(layers)
    bundle = _ca_bundle(layers)
    for layer in layers:
        for name, relative in sorted(layer.links.items()):
            target = layer.spec.container_dir / relative
            link = LINK_DIR / name
            if name in KEEP_IMAGE_LINKS:
                parts.append(f"{{ [ -e {link} ] || ln -sf {target} {link}; }}")
            elif "node" in (layer.manifest.get("requires") or []) and node:
                roots = (
                    f'[ -n "${CODEX_CA_VARIABLE}" ] || '
                    f"export {CODEX_CA_VARIABLE}={bundle}\\n"
                    if bundle and layer.spec.name == "codex"
                    else ""
                )
                parts.append(
                    f"rm -f {link} && printf '#!/bin/sh\\n{roots}exec {node} {target} "
                    f"\"$@\"\\n' > {link} && chmod 755 {link}"
                )
            else:
                parts.append(f"ln -sf {target} {link}")
    return " && ".join(parts)


def version_executable(layer: Layer, name: str) -> PurePosixPath:
    """What the version check runs: the layer's own binary for Node's
    tools, which may not be the ones on the PATH, else the linked name."""
    if name in KEEP_IMAGE_LINKS and name in layer.links:
        return layer.spec.container_dir / layer.links[name]
    return LINK_DIR / name


def setup_record(
    *,
    mode: str,
    platform: str | None,
    layers: list[Layer],
    phases_ms: dict[str, int],
    versions: dict[str, str],
    note: str | None = None,
) -> dict[str, Any]:
    """The per-trial setup record the attempt record and the Gym read."""
    caches = {layer.cache for layer in layers}
    return {
        "schema": SETUP_SCHEMA,
        "mode": mode,
        "cache": "none"
        if not layers
        else ("warm" if caches == {"warm"} else "cold"),
        "platform": platform,
        "layers": [
            {
                "key": layer.spec.key,
                "cache": layer.cache,
                "build_ms": layer.build_ms if layer.cache == "cold" else 0,
                "tree_sha256": layer.manifest.get("tree_sha256"),
                "bytes": layer.manifest.get("bytes"),
            }
            for layer in layers
        ],
        "phases_ms": phases_ms,
        "install_ms": sum(phases_ms.values()),
        "versions": versions,
        "boundary": (
            "install_ms covers the toolchain only (host layer build or "
            "reuse, copy, link, and version checks); Harbor's agent_setup "
            "phase also covers the Coder One artifact upload and the "
            "episode doctor."
        ),
        "note": note,
    }


PLATFORM_PROBE = (
    "uname -m; "
    "if ldd --version 2>&1 | grep -qi musl || [ -f /etc/alpine-release ]; "
    "then echo musl; else echo glibc; fi"
)


async def place_toolchain(
    agent: Any,
    environment: Any,
    executor: str,
    version: str,
    *,
    root: Path | None = None,
    fetch: Fetch = _fetch,
    credentials: dict[str, str] | None = None,
) -> dict[str, Any]:
    """Copy the executor's cached layers into the environment and link them.

    ``agent`` is the Harbor installed agent (for ``exec_as_root``). Raises
    ``ToolchainError`` when no layer fits the environment, so the caller
    can fall back to the network install and say why.
    """
    import asyncio

    phases: dict[str, int] = {}

    def lap(name: str, started: float) -> float:
        now = time.monotonic()
        phases[name] = int((now - started) * 1000)
        return now

    clock = time.monotonic()
    probe = await environment.exec(command=PLATFORM_PROBE, user="root")
    words = (probe.stdout or "").split()
    if probe.return_code != 0 or len(words) < 2:
        raise ToolchainError(f"couldn't read the environment's platform: {probe.stderr}")
    platform = platform_for(words[0], words[-1] == "musl")
    clock = lap("probe", clock)
    layers = [
        await asyncio.to_thread(
            ensure_layer, spec, root=root, fetch=fetch, credentials=credentials
        )
        for spec in layers_for(executor, version, platform)
    ]
    clock = lap("host_layers", clock)
    await agent.exec_as_root(
        environment,
        command="mkdir -p "
        + " ".join(str(layer.spec.container_dir) for layer in layers),
    )
    for layer in layers:
        await environment.upload_dir(str(layer.root), str(layer.spec.container_dir))
    clock = lap("copy", clock)
    await agent.exec_as_root(environment, command=link_command(layers))
    clock = lap("link", clock)
    versions: dict[str, str] = {}
    for layer in layers:
        if "version_command" not in layer.manifest:
            continue
        command = layer.manifest["version_command"]
        name = command.split()[0]
        executable = version_executable(layer, name)
        found = await environment.exec(command=f"{executable} {command.split(' ', 1)[1]}")
        text = (found.stdout or "").strip()
        expected = layer.manifest["expected_version"]
        if found.return_code != 0 or expected not in text.split():
            raise ToolchainError(
                f"{layer.spec.key}: `{command}` printed {text[-200:]!r}, "
                f"not {expected}"
            )
        versions[layer.spec.name] = expected
    lap("verify", clock)
    return setup_record(
        mode="prebuilt",
        platform=platform,
        layers=layers,
        phases_ms=phases,
        versions=versions,
    )


def check_image(
    image: str,
    executor: str,
    version: str,
    *,
    docker: str = "docker",
    timeout: int = 600,
) -> dict[str, Any]:
    """Place an executor's layers in ``image`` and run their version checks.

    What a trial's prebuilt install does, without Harbor or a task: probe
    the image's platform, build or reuse the layers on the host, mount them
    read-only where a trial copies them, link them, and run each layer's
    version command. The container has no network. Pulls the image if the
    host doesn't have it.
    """
    import subprocess

    def run(args: list[str]) -> tuple[int, str]:
        try:
            done = subprocess.run(
                [docker, "run", "--rm", "--network", "none", "--user", "root",
                 "--entrypoint", "sh", *args],
                capture_output=True,
                text=True,
                timeout=timeout,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            return 127, str(exc)
        return done.returncode, (done.stdout or "") + (done.stderr or "")

    code, text = run([image, "-c", PLATFORM_PROBE + "; (ldd --version 2>&1 | head -n1) || true"])
    lines = text.strip().splitlines()
    if code != 0 or len(lines) < 2:
        return {"ok": False, "platform": None, "detail": f"probe failed: {text.strip()[-300:]}"}
    libc_line = lines[2] if len(lines) > 2 else "unknown C library"
    try:
        platform = platform_for(lines[0], lines[1].strip() == "musl")
        layers = [ensure_layer(spec) for spec in layers_for(executor, version, platform)]
    except ToolchainError as exc:
        return {"ok": False, "platform": None, "detail": f"{exc}; a trial falls back to the network install"}
    mounts: list[str] = []
    for layer in layers:
        mounts += ["-v", f"{layer.root}:{layer.spec.container_dir}:ro"]
    checks = []
    versioned = [layer for layer in layers if "version_command" in layer.manifest]
    for layer in versioned:
        command = layer.manifest["version_command"]
        name = command.split()[0]
        checks.append(f"{version_executable(layer, name)} {command.split(' ', 1)[1]}")
    script = link_command(layers) + " && " + " && ".join(checks)
    code, text = run([*mounts, image, "-c", script])
    words = text.split()
    missing = [
        layer.manifest["expected_version"]
        for layer in versioned
        if layer.manifest["expected_version"] not in words
    ]
    ok = code == 0 and not missing
    detail = libc_line.strip()
    if not ok:
        detail += f"; version check failed: {text.strip()[-300:]}"
    return {"ok": ok, "platform": platform, "detail": detail}


def write_setup(logs_dir: Path, record: dict[str, Any]) -> None:
    """Keep the setup record beside the agent's other logs."""
    try:
        (Path(logs_dir) / "toolchain-setup.json").write_text(
            json.dumps(record, indent=2) + "\n"
        )
    except OSError:
        pass
