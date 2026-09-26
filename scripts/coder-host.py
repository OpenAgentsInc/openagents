#!/usr/bin/env python3
"""Install infrastructure for immutable Coder bundles and explicit task services."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import selectors
import time
import shutil
import stat
import subprocess
import sys
import tempfile

SCHEMA = "openagents.coder.host-install.v1"
STATE = "openagents.coder.task-store.v2"


def refuse(message):
    raise ValueError(message)


def digest(path):
    h = hashlib.sha256()
    with ordinary(path, "rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def ordinary(path, mode):
    flags = os.O_RDONLY if mode == "rb" else os.O_WRONLY
    fd = os.open(path, flags | os.O_NOFOLLOW | os.O_NONBLOCK)
    m = os.fstat(fd)
    if not stat.S_ISREG(m.st_mode) or m.st_nlink != 1:
        os.close(fd)
        refuse("expected an ordinary, unlinked file")
    return os.fdopen(fd, mode)


def read_json(path):
    with ordinary(path, "rb") as f:
        raw = f.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        refuse("host metadata exceeds its byte limit")
    def pairs(items):
        d = {}
        for k, v in items:
            if k in d:
                refuse("duplicate metadata field")
            d[k] = v
        return d
    return json.loads(raw, object_pairs_hook=pairs)


def sync_dir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def atomic(path, content):
    fd, temporary = tempfile.mkstemp(prefix=".pending-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(content)
            f.flush()
            os.fsync(f.fileno())
        os.replace(temporary, path)
        sync_dir(path.parent)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def json_bytes(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def health(binary):
    environment = {"PATH": "/usr/bin:/bin", "HOME": str(binary.parent)}
    results = {}
    for args, key in [(["--version"], "version"), (["task", "--help"], "tasks")]:
        process = subprocess.Popen([str(binary), *args], env=environment,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        value = bytearray()
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        deadline = time.monotonic() + 10
        try:
            while selector.get_map():
                left = deadline - time.monotonic()
                if left <= 0:
                    refuse("binary health check timed out")
                for key_event, _ in selector.select(left):
                    chunk = os.read(key_event.fd, 4096)
                    if not chunk:
                        selector.unregister(key_event.fileobj)
                    value.extend(chunk)
                    if len(value) > 16384:
                        refuse("binary health check exceeded its output bound")
            result = process.wait(timeout=max(0.01, deadline - time.monotonic()))
            if result != 0:
                refuse("binary health check failed")
        finally:
            selector.close()
            # The direct child may exit while a descendant remains alive.
            # Health checks own their entire fresh group until cleanup.
            try:
                os.killpg(process.pid, 9)
            except ProcessLookupError:
                pass
            process.wait()
            process.stdout.close()
        text = value.decode("utf-8", errors="strict")
        if key == "tasks" and "task execute" not in text:
            refuse("binary does not expose the durable task execution interface")
        results[key] = text.strip() if key == "version" else "task execute advertised"
    return results


class Installation:
    def __init__(self, root, create=False):
        self.root = Path(root).absolute()
        fresh = False
        if not create and not self.root.exists():
            refuse("host installation is unavailable")
        try:
            self.root.mkdir(mode=0o700)
            sync_dir(self.root.parent)
            fresh = True
        except FileExistsError:
            pass
        if fresh and not create:
            self.root.rmdir()
            refuse("host installation is unavailable")
        m = self.root.lstat()
        if not stat.S_ISDIR(m.st_mode) or m.st_mode & 0o077:
            refuse("host root must be a private ordinary directory")
        self.root = self.root.resolve()
        lock = self.root / "host.lock"
        if not fresh and (not lock.exists() or not (self.root / "active.json").exists()):
            refuse("host installation is incomplete; preserve it for recovery")
        fd = os.open(lock, os.O_RDWR | os.O_NOFOLLOW | (os.O_CREAT | os.O_EXCL if fresh else 0), 0o600)
        self.lock = os.fdopen(fd, "r+b")
        m = os.fstat(fd)
        if not stat.S_ISREG(m.st_mode) or m.st_nlink != 1 or m.st_mode & 0o077:
            refuse("unsafe host installation lock")
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            refuse("host installation is busy")
        if fresh:
            (self.root / "versions").mkdir(mode=0o700)
            (self.root / "grants").mkdir(mode=0o700)
            (self.root / "logs").mkdir(mode=0o700)
            self.document = {"schema": SCHEMA, "active": None, "previous": None, "history": []}
            self.save()
        self.document = read_json(self.root / "active.json")
        if set(self.document) != {"schema", "active", "previous", "history"} or self.document["schema"] != SCHEMA:
            refuse("unsupported installation state")
        if not isinstance(self.document["history"], list) or len(self.document["history"]) > 1000:
            refuse("invalid installation history")
        for name in ["versions", "grants", "logs"]:
            m = (self.root / name).lstat()
            if not stat.S_ISDIR(m.st_mode) or m.st_mode & 0o077:
                refuse("unsafe installation subdirectory")

    def __del__(self):
        if hasattr(self, "lock"):
            self.lock.close()

    def save(self):
        held = os.fstat(self.lock.fileno())
        path = (self.root / "host.lock").lstat()
        if (held.st_dev, held.st_ino, held.st_nlink) != (path.st_dev, path.st_ino, 1):
            refuse("installation lock changed")
        if len(self.document["history"]) > 1000:
            refuse("installation history limit reached")
        atomic(self.root / "active.json", json_bytes(self.document))

    def release(self, identity):
        if not isinstance(identity, str) or not re.fullmatch(r"[0-9a-f]{64}", identity):
            refuse("invalid bundle identity")
        directory = self.root / "versions" / identity
        if not stat.S_ISDIR(directory.lstat().st_mode):
            refuse("bundle directory is not ordinary")
        manifest = read_json(directory / "manifest.json")
        if manifest.get("schema") != "openagents.coder.host-bundle.v1" or manifest.get("binary_sha256") != identity:
            refuse("bundle manifest identity mismatch")
        if digest(directory / "coder") != identity:
            refuse("retained binary digest mismatch")
        return directory / "coder", manifest

    def compatible(self, manifest, tasks):
        tasks = Path(tasks)
        if tasks.exists() and not stat.S_ISDIR(tasks.lstat().st_mode):
            refuse("task directory is not ordinary")
        state = tasks / "tasks.json"
        if state.exists() or state.is_symlink():
            schema = read_json(state).get("schema")
            if schema not in manifest["read_state_schemas"]:
                refuse("retained task state is incompatible with the selected binary")

    def select(self, identity, tasks, operation):
        _, manifest = self.release(identity)
        self.compatible(manifest, tasks)
        if self.document["active"] == identity:
            return "unchanged"
        self.document["previous"], self.document["active"] = self.document["active"], identity
        self.document["history"].append({"operation": operation, "binary_sha256": identity})
        self.save()
        return "activated"


def install(host, args):
    binary = Path(args.binary).absolute()
    if not re.fullmatch(r"[0-9a-f]{64}", args.sha256) or digest(binary) != args.sha256:
        refuse("input binary digest mismatch")
    if not re.fullmatch(r"[0-9a-f]{40}", args.source_revision):
        refuse("source revision must be a full Git commit")
    schemas = sorted(set(args.read_state_schema or [STATE]))
    if not schemas or any(not re.fullmatch(r"openagents\.coder\.task-store\.v[0-9]+", s) for s in schemas):
        refuse("invalid readable task schema")
    destination = host.root / "versions" / args.sha256
    manifest = {"schema": "openagents.coder.host-bundle.v1", "binary_sha256": args.sha256,
                "source_revision": args.source_revision, "uncommitted_source": args.uncommitted_source,
                "read_state_schemas": schemas, "platform": sys.platform}
    host.compatible(manifest, args.tasks)
    if destination.exists():
        _, previous = host.release(args.sha256)
        for key in manifest:
            if previous.get(key) != manifest[key]:
                refuse("an immutable bundle cannot acquire different provenance")
    else:
        needed = binary.stat().st_size * 2 + 16 * 1024 * 1024
        if shutil.disk_usage(host.root).free < needed:
            refuse("insufficient free space for staged bundle and recovery reserve")
        temporary = Path(tempfile.mkdtemp(prefix=".staging-", dir=host.root / "versions"))
        try:
            with ordinary(binary, "rb") as source, (temporary / "coder").open("xb") as out:
                shutil.copyfileobj(source, out)
                out.flush()
                os.fsync(out.fileno())
            (temporary / "coder").chmod(0o500)
            if digest(temporary / "coder") != args.sha256:
                refuse("binary changed while staging")
            manifest["health"] = health(temporary / "coder")
            atomic(temporary / "manifest.json", json_bytes(manifest))
            sync_dir(temporary)
            os.rename(temporary, destination)
            sync_dir(destination.parent)
        finally:
            if temporary.exists():
                shutil.rmtree(temporary)
    result = host.select(args.sha256, args.tasks, "install")
    return {"result": result, "binary_sha256": args.sha256}


def service(host, args):
    binary, manifest = host.release(host.document["active"])
    host.compatible(manifest, args.tasks)
    grant = read_json(Path(args.grant))
    if grant.get("schema") != "openagents.coder.task-execution-grant.v1":
        refuse("service needs an explicit versioned task execution grant")
    if not re.fullmatch(r"[a-zA-Z0-9._-]{1,80}", args.label):
        refuse("invalid service label")
    wall = grant.get("wall_seconds")
    if type(wall) is not int or not 1 <= wall <= 3600:
        refuse("invalid execution wall limit")
    content = json_bytes(grant)
    identity = hashlib.sha256(content).hexdigest()
    retained = host.root / "grants" / (identity + ".json")
    if retained.exists():
        if digest(retained) != identity:
            refuse("retained service grant changed")
    else:
        atomic(retained, content)
    argv = [str(binary), "task", "execute", "--grant", str(retained), "--store", str(Path(args.tasks).absolute())]
    if args.platform == "macos":
        document = {"Label": args.label, "ProgramArguments": argv, "RunAtLoad": True,
                    "KeepAlive": False, "AbandonProcessGroup": False, "ExitTimeOut": 10,
                    "ProcessType": "Background", "Umask": 0o077,
                    "StandardOutPath": str(host.root / "logs" / (args.label + ".out")),
                    "StandardErrorPath": str(host.root / "logs" / (args.label + ".err")),
                    "EnvironmentVariables": {"PATH": "/usr/bin:/bin"}}
        return plistlib.dumps(document).decode()
    def quote(value):
        if any(c in value for c in "\r\n\x00"):
            refuse("service arguments contain an unsupported control character")
        return '"' + value.replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%').replace('$', '$$') + '"'
    return "\n".join(["[Unit]", "Description=Explicit bounded Coder task", "", "[Service]",
                      "Type=oneshot", "ExecStart=" + " ".join(map(quote, argv)), "Restart=no",
                      "RemainAfterExit=yes", "KillMode=control-group", "TimeoutStopSec=10",
                      f"TimeoutStartSec={wall + 30}", "UMask=0077", "NoNewPrivileges=yes",
                      "Environment=PATH=/usr/bin:/bin", "", "[Install]", "WantedBy=default.target", ""])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, help="private installation directory")
    parser.add_argument("--tasks", required=True, help="separate retained task directory")
    subs = parser.add_subparsers(dest="operation", required=True)
    p = subs.add_parser("install")
    p.add_argument("--binary", required=True)
    p.add_argument("--sha256", required=True)
    p.add_argument("--source-revision", required=True)
    p.add_argument("--uncommitted-source", action="store_true")
    p.add_argument("--read-state-schema", action="append")
    subs.add_parser("doctor")
    subs.add_parser("rollback")
    subs.add_parser("uninstall")
    p = subs.add_parser("service")
    p.add_argument("--platform", choices=["macos", "linux"], required=True)
    p.add_argument("--grant", required=True)
    p.add_argument("--label", required=True)
    args = parser.parse_args()
    try:
        host = Installation(args.root, create=args.operation == "install")
        if args.operation == "install":
            result = install(host, args)
        elif args.operation == "service":
            sys.stdout.write(service(host, args))
            return
        elif args.operation == "rollback":
            previous = host.document["previous"]
            if previous is None:
                refuse("no retained rollback bundle")
            binary, _ = host.release(previous)
            health(binary)
            result = {"result": host.select(previous, args.tasks, "rollback"), "binary_sha256": previous}
        elif args.operation == "uninstall":
            host.document["previous"] = host.document["active"]
            host.document["active"] = None
            host.document["history"].append({"operation": "uninstall", "task_data": "preserved"})
            host.save()
            result = {"result": "deactivated", "task_data": "preserved", "service_action": "unload separately; existing rendered services remain pinned"}
        else:
            binary, manifest = host.release(host.document["active"])
            host.compatible(manifest, args.tasks)
            result = {"result": "available", "binary": str(binary), "bundle": manifest,
                      "health": health(binary), "task_store": str(Path(args.tasks).absolute()),
                      "execution": "requires a separate grant", "automatic_restart": False}
        print(json.dumps(result, sort_keys=True))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(json.dumps({"error": str(error)}), file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
