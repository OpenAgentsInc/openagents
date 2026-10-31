"""New bridge effect-order contexts, using existing qualified C/header emits.

No SDK, neural inference, renderer or previous numerical gate is executed.
"""
from pathlib import Path
import argparse, hashlib, json, os, platform, re, shutil, subprocess, time

ROOT = Path(__file__).resolve().parents[3]
HERE = ROOT / "psionic/tests/robotics"
NAMES = ["pk_forward", "pk_sample", "pk_control", "pqs_select", "ps_sample", "pqn_model", "pqn_asset_path"]

def identity(path):
    data = Path(path).read_bytes()
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}

def source_inventory():
    pending = [HERE / "panda-query-runtime.coax"]
    found = {}
    while pending:
        path = pending.pop().resolve()
        if str(path.relative_to(ROOT)) in found:
            continue
        found[str(path.relative_to(ROOT))] = identity(path)
        pending.extend((path.parent / p).resolve() for p in re.findall(r'#load\s+"([^"]+)"', path.read_text()))
    for path in [Path(__file__), HERE / "panda_query_runtime_mock.c", HERE / "panda_query_runtime.h", ROOT / "psionic/platform/panda-env.h"]:
        found[str(path.relative_to(ROOT))] = identity(path)
    return found

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", type=Path, required=True, help="Existing debug/release/sanitized panda-session.c/.h directory")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--reuse-debug-object", type=Path, help="Reuse an unchanged generated-C object from a recorded earlier attempt")
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise RuntimeError("Unqualified native target; this is a Darwin-arm64 effect-order qualification")
    out = args.output.resolve()
    if out.exists():
        raise RuntimeError("Preserve previous/partial records; choose a new context directory")
    out.mkdir(parents=True)
    inventory = source_inventory()
    record = {"schema": "psionic.panda-query-runtime.effect-order.v1", "passed": False, "source_inventory": inventory,
              "reused_C_header_artifacts": {}, "commands": [], "profiles": {}, "SDK_calls": 0, "NN_calls": 0, "steps": 0,
              "scope": "13 new mocked control contexts and 2 preflight refusals; actual Coax functions; no old numeric gates"}
    def save():
        (out / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    def run(name, argv):
        start = time.monotonic()
        result = subprocess.run([str(x) for x in argv], cwd=ROOT, capture_output=True, timeout=180)
        (out / (name + ".stdout")).write_bytes(result.stdout)
        (out / (name + ".stderr")).write_bytes(result.stderr)
        record["commands"].append({"name": name, "argv": [str(x) for x in argv], "exit": result.returncode,
                                   "seconds": time.monotonic() - start, "stdout": identity(out / (name + ".stdout")), "stderr": identity(out / (name + ".stderr"))})
        save()
        if result.returncode or result.stderr:
            raise RuntimeError((name, result.stdout.decode(errors="replace"), result.stderr.decode(errors="replace")))
        return result.stdout
    save()
    try:
        for profile in ["debug", "release", "sanitized"]:
            directory = out / profile
            directory.mkdir()
            cached = args.artifacts.resolve() / profile
            record["reused_C_header_artifacts"][profile] = {}
            for name in ["panda-session.c", "panda-session.h"]:
                original = cached / name
                record["reused_C_header_artifacts"][profile][name] = identity(original)
                shutil.copyfile(original, directory / name)
            flags = ["-std=c11", "-D_DARWIN_C_SOURCE", "-Wall", "-Wextra", "-Werror", "-Wno-unused-function", "-Wno-unused-parameter", "-Wno-unused-variable", "-fno-fast-math", "-ffp-contract=off", "-fno-builtin-sin", "-fno-builtin-cos"]
            flags += ["-O0", "-g"] if profile == "debug" else ["-O2"]
            if profile == "sanitized":
                flags += ["-g", "-fsanitize=address,undefined", "-fno-sanitize-recover=all", "-fno-omit-frame-pointer"]
            includes = ["-I" + str(directory), "-I" + str(HERE), "-I" + str(ROOT / "psionic/platform")]
            cc = os.environ.get("CC", "cc")
            if profile == "debug" and args.reuse_debug_object:
                prior_object = args.reuse_debug_object.resolve()
                prior_record = prior_object.parent.parent / "record.json"
                prior = json.loads(prior_record.read_text())
                if (prior["reused_C_header_artifacts"]["debug"] != record["reused_C_header_artifacts"]["debug"]
                        or not any(c["name"] == "debug-object" and c["exit"] == 0 for c in prior["commands"])):
                    raise RuntimeError("Earlier object has no matching successful generated-C compilation")
                for name in ["panda-session.c", "panda-session.h"]:
                    if identity(prior_object.parent / name) != identity(directory / name):
                        raise RuntimeError("Earlier object C/header changed")
                shutil.copyfile(prior_object, directory / "session.o")
                record["reused_debug_object"] = {"object": identity(prior_object), "record": identity(prior_record), "path": str(prior_object)}
                save()
            else:
                run(profile + "-object", [cc, *flags, *includes, *["-D" + n + "=mock_real_" + n for n in NAMES], "-c", directory / "panda-session.c", "-o", directory / "session.o"])
            run(profile + "-link", [cc, *flags, *includes, HERE / "panda_query_runtime_mock.c", directory / "session.o", "-lm", "-o", directory / "mock"])
            report = json.loads(run(profile + "-run", [directory / "mock"]))
            if not report["passed"] or report["control_cases"] != 13 or report["preflight_cases"] != 2 or any(report[k] != 0 for k in ["SDK_calls", "NN_calls", "steps"]):
                raise RuntimeError("Unexpected work scope")
            record["profiles"][profile] = report
            save()
        if source_inventory() != inventory:
            raise RuntimeError("Inputs changed during qualification")
        record["passed"] = True
    except BaseException as error:
        record["failure"] = repr(error)
        raise
    finally:
        save()
    print(json.dumps({"passed": True, "record": str(out / "record.json"), "profiles": record["profiles"]}))

if __name__ == "__main__":
    main()
