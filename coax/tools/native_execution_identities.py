"""Mechanical compiler/SDK observations for optional native execution tooling.

No field is a qualification verdict. The Coax policy admits required process
observations; SDK path/version and settings hashes are not an SDK-content lock.
"""
import errno
import hashlib
import os
from pathlib import Path
import platform
import shutil
import stat


_SETTINGS = ("SDKROOT", "DEVELOPER_DIR", "MACOSX_DEPLOYMENT_TARGET", "COAX_HOST_TARGET")


def _hash(path):
    if not stat.S_ISREG(Path(path).stat().st_mode):
        raise OSError(errno.EINVAL, "not a regular file", os.fspath(path))
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _line(observation):
    """Transport a successful query's bytes as its reported filesystem text."""
    raw = observation.get("stdout")
    if observation.get("returncode") != 0 or not isinstance(raw, bytes):
        return None
    return os.fsdecode(raw.rstrip(b"\r\n"))


def inspect(cc_argv, runner, *, cwd=None, environment=None):
    """Return raw facts in the supplied runner context, without mutating it."""
    argv = [os.fspath(part) for part in cc_argv]
    lookup_environment = os.environ if environment is None else environment
    result = {
        "argv": argv, "resolved_program": None, "program_sha256": None,
        "resolution_error": None, "version_observation": None,
        "target_observation": None,
        "settings": {name: lookup_environment.get(name) for name in _SETTINGS},
        "sdk": {"platform": platform.system(), "path_observation": None,
                "version_observation": None, "path": None, "resolved_path": None,
                "path_exists": False, "version": None, "settings_files": [],
                "environment": {}},
    }
    if not argv:
        result["resolution_error"] = "no compiler argv0 was supplied"
        return result
    try:
        working_directory = Path.cwd() if cwd is None else Path(cwd).resolve()
        program = argv[0]
        if os.path.dirname(program):
            target = Path(program)
            resolved = (target if target.is_absolute() else working_directory / target).resolve(strict=True)
        else:
            search = []
            for entry in os.get_exec_path(lookup_environment):
                directory = Path(entry)
                search.append(os.fspath(directory if directory.is_absolute() else working_directory / directory))
            found = shutil.which(program, path=os.pathsep.join(search))
            if found is None:
                raise FileNotFoundError(errno.ENOENT, os.strerror(errno.ENOENT), program)
            resolved = Path(found).resolve(strict=True)
        result["resolved_program"] = str(resolved)
        result["program_sha256"] = _hash(resolved)
    except OSError as error:
        result["resolution_error"] = str(error)

    def query(command):
        return runner(command, timeout_s=5.0, max_output_bytes=65536, quiet_faults=True)

    result["version_observation"] = query([*argv, "--version"])
    result["target_observation"] = query([*argv, "-dumpmachine"])
    if result["sdk"]["platform"] != "Darwin":
        return result

    sdk = result["sdk"]
    sdk["path_observation"] = query(["xcrun", "--sdk", "macosx", "--show-sdk-path"])
    sdk["version_observation"] = query(["xcrun", "--sdk", "macosx", "--show-sdk-version"])
    sdk["path"] = _line(sdk["path_observation"])
    sdk["version"] = _line(sdk["version_observation"])
    if sdk["path"]:
        try:
            sdk_path = Path(sdk["path"])
            sdk["path_exists"] = sdk_path.is_dir()
            if sdk["path_exists"]:
                sdk["resolved_path"] = str(sdk_path.resolve(strict=True))
                for name in ("SDKSettings.json", "SDKSettings.plist"):
                    settings = sdk_path / name
                    if not settings.is_file():
                        continue
                    fact = {"path": str(settings), "resolved_path": None,
                            "sha256": None, "error": None}
                    try:
                        fact["resolved_path"] = str(settings.resolve(strict=True))
                        fact["sha256"] = _hash(settings)
                    except OSError as error:
                        fact["error"] = str(error)
                    sdk["settings_files"].append(fact)
                # A candidate environment from observed existing SDK/version
                # facts. The caller still asks Coax to admit the query results.
                if sdk["version"]:
                    sdk["environment"] = {"SDKROOT": str(sdk_path)}
        except OSError as error:
            sdk["path_error"] = str(error)
    return result
