#!/usr/bin/env python3
"""Reconstruct the quantity compiler privately from checked-in normal seeds.

This optional developer tool runs the focused seed -> graph -> compiler ->
graph C/header fixed point. It does not run the complete bootstrap release
suite, qualify a cache/provenance record, install a compiler, or write sources.

--plan reads files and Git metadata only: no subprocesses or filesystem writes.
--verify-receipt checks the normalized command plan and selected source/seed
identities; it does not independently recompute the full normal fingerprint.
All execution shares one 840-second wall budget; partial output is retained.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import time

REPO_ROOT = Path(__file__).resolve().parents[2]
WALL_BUDGET_SECONDS = 840
AUTHORED = ("compiler", "parser", "checker", "quantity")


class BootstrapError(Exception):
    pass


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def commit_from_files(root):
    """Read HEAD/refs without invoking Git or changing repository state."""
    dot = root / ".git"
    if dot.is_file():
        value = dot.read_text(encoding="utf-8").strip()
        if not value.startswith("gitdir: "):
            return None
        gitdir = (root / value[8:]).resolve()
    elif dot.is_dir():
        gitdir = dot
    else:
        return None
    try:
        value = (gitdir / "HEAD").read_text(encoding="utf-8").strip()
        if re.fullmatch(r"[0-9a-fA-F]{40}|[0-9a-fA-F]{64}", value):
            return value.lower()
        if not value.startswith("ref: "):
            return None
        ref = value[5:]
        if not ref.startswith("refs/") or ".." in ref.split("/"):
            return None
        common = gitdir
        if (gitdir / "commondir").is_file():
            common = (gitdir / (gitdir / "commondir").read_text().strip()).resolve()
        for directory in (gitdir, common):
            loose = directory / ref
            if loose.is_file():
                candidate = loose.read_text().strip()
                if re.fullmatch(r"[0-9a-fA-F]{40}|[0-9a-fA-F]{64}", candidate):
                    return candidate.lower()
            packed = directory / "packed-refs"
            if packed.is_file():
                for line in packed.read_text().splitlines():
                    fields = line.split()
                    if len(fields) == 2 and fields[1] == ref:
                        if re.fullmatch(r"[0-9a-fA-F]{40}|[0-9a-fA-F]{64}", fields[0]):
                            return fields[0].lower()
    except OSError:
        return None
    return None


def inside(path, directory):
    return path == directory or directory in path.parents


def private_output(root, requested, plan_only):
    output = requested.resolve()
    if inside(output, root):
        raise BootstrapError("output must be outside the source repository")
    caches = [Path.home() / ".cache/coax-release"]
    if os.environ.get("COAX_RELEASE_CACHE"):
        caches.append(Path(os.environ["COAX_RELEASE_CACHE"]))
    if any(inside(output, cache.expanduser().resolve()) for cache in caches):
        raise BootstrapError("output must be private, outside release caches")
    if not plan_only and (requested.exists() or requested.is_symlink() or output.exists()):
        raise BootstrapError("output directory must be new; existing output is never overwritten")
    return output


def plan_for(root, work, source_commit):
    coax = root / "coax"
    recipe_path = coax / "bootstrap-build.sh"
    if not recipe_path.is_file():
        raise BootstrapError("--root must contain coax/bootstrap-build.sh")
    recipe = recipe_path.read_text(encoding="utf-8")
    required = (
        "host_flags=(-std=c++20 -O1 -g -Wall -Wextra -Wno-missing-field-initializers)",
        "c_flags=(-std=c11 -O1 -g)",
    )
    if any(line not in recipe for line in required):
        raise BootstrapError("normal bootstrap compiler flags changed; review this focused recipe")
    seeds = re.findall(r'-c bootstrap/([a-z0-9-]+)\.c -o "\$work/seed-[a-z0-9-]+\.o"', recipe)
    lines = [line for line in recipe.splitlines() if "-DCOAX_BOOTSTRAP_IMPORTER" in line and "seed-" in line]
    if len(lines) != 1:
        raise BootstrapError("normal stage0 link recipe was not recognized")
    link_seeds = re.findall(r"\$work/seed-([a-z0-9-]+)\.o", lines[0])
    if not seeds or len(seeds) != len(set(seeds)) or len(link_seeds) != len(seeds) or set(seeds) != set(link_seeds):
        raise BootstrapError("normal stage0 seed compile/link lists disagree")
    if '-I bootstrap -I "$work"' not in lines[0]:
        raise BootstrapError("normal checked-in-header include priority changed")
    cc, cxx = os.environ.get("CC", "cc"), os.environ.get("CXX", "c++")
    cflags = ["-std=c11", "-O1", "-g"]
    cppflags = ["-std=c++20", "-O1", "-g", "-Wall", "-Wextra", "-Wno-missing-field-initializers"]
    if os.environ.get("COAX_HOST_TARGET"):
        target = ["-target", os.environ["COAX_HOST_TARGET"]]
        cflags += target
        cppflags += target
    commands = []

    def add(name, arguments, output=None):
        commands.append({
            "name": name, "arguments": [str(a) for a in arguments], "cwd": str(coax),
            "stdout_path": str(output or work / (name + ".stdout")),
            "stderr_path": str(work / (name + ".stderr")),
        })

    add("fingerprint-before", ["./provenance.sh", "fingerprint", root])
    for name in seeds:
        add("seed-" + name, [cc, *cflags, "-c", f"bootstrap/{name}.c", "-o", work / f"seed-{name}.o"])
    add("stage0-cxx", [cxx, *cppflags, "-DCOAX_BOOTSTRAP_IMPORTER", "-I", "bootstrap", "-I", work,
                       "-o", work / "stage0", "src/main.cpp", *[work / f"seed-{name}.o" for name in link_seeds]])
    add("stage0-graph", [work / "stage0", "c", "lib/compiler/compiler.coax", "--library", "--header", work / "proto_importer.h"], work / "proto_importer.c")
    add("graph-object", [cc, *cflags, "-c", work / "proto_importer.c", "-o", work / "proto_importer.o"])
    add("final-cxx", [cxx, *cppflags, "-I", work, "-o", work / "coax", "src/main.cpp", work / "proto_importer.o"])
    add("final-version", [work / "coax", "version"])
    add("final-graph", [work / "coax", "c", "lib/compiler/compiler.coax", "--library", "--header", work / "rebuilt.h"], work / "rebuilt.c")
    add("graph-c-fixedpoint", ["cmp", work / "proto_importer.c", work / "rebuilt.c"])
    add("graph-header-fixedpoint", ["cmp", work / "proto_importer.h", work / "rebuilt.h"])
    for name in ("parser", "checker"):
        add("final-seed-" + name, [work / "coax", "c", f"lib/compiler/{name}.coax", "--library", "--header", work / f"{name}-final.h"], work / f"{name}-final.c")
        add(name + "-seed-c-match", ["cmp", f"bootstrap/{name}.c", work / f"{name}-final.c"])
        add(name + "-seed-header-match", ["cmp", f"bootstrap/{name}.h", work / f"{name}-final.h"])
    add("fingerprint-after", ["./provenance.sh", "fingerprint", root])
    protocol = b""
    for name, relative, tag in (("CORE", "wire", "COAX_CORE"), ("MEMORY", "memory", "COAX_MEMORY"),
                                ("IMPORTER", "importer", "COAX_IMPORTER"), ("FINGERPRINT", "fingerprint", "COAX_FP")):
        protocol += f'static constexpr char PROTO_{name}_SOURCE[] = R"{tag}('.encode()
        if relative == "fingerprint":  # the fingerprint source is the SHA-256 kernel, then fingerprint.coax
            protocol += (coax / "lib/crypto/sha256-kernel.coax").read_bytes()
        protocol += (coax / f"lib/proto/{relative}.coax").read_bytes()
        protocol += f'){tag}";\n'.encode()
    plan = {
        "schema": "coax.quantity.bootstrap.plan.v1", "root": str(root), "working_directory": str(coax),
        "output_directory": str(work), "source_commit": source_commit,
        "source_commit_limit": "HEAD is a label, not a clean-tree assertion; source hashes and normal fingerprints bind actual inputs.",
        "normal_bootstrap_recipe_sha256": sha(recipe_path),
        "normal_seed_compile_order": seeds, "normal_seed_link_order": link_seeds,
        "seed_sha256": {f"coax/bootstrap/{name}.{ext}": sha(coax / f"bootstrap/{name}.{ext}") for name in seeds for ext in ("c", "h")},
        "authored_source_sha256": {f"coax/lib/compiler/{name}.coax": sha(coax / f"lib/compiler/{name}.coax") for name in AUTHORED},
        "embedded_protocol_sources_sha256": hashlib.sha256(protocol).hexdigest(),
        "preparation": {"generated_file": str(work / "proto_runtime.inc"), "source": "Four exact source literals (lib/proto, with lib/crypto/sha256-kernel.coax before fingerprint.coax), with the normal raw-string delimiters."},
        "commands": commands, "command_count": len(commands), "shared_wall_budget_seconds": WALL_BUDGET_SECONDS,
        "normal_rebuild_command": "coax/build.sh", "normal_rebuild_executed": False,
        "scope": "Focused checked-in-seed reconstruction and C/header fixed point, not the complete normal release/cache/provenance qualification.",
        "source_writes": False, "cache_or_installed_release_published": False,
    }
    return plan, protocol


def normalized_commands(record, root, work):
    roots = sorted(((str(root), "${ROOT}"), (str(work), "${WORK}")), key=lambda pair: -len(pair[0]))

    def replace(value):
        for path, token in roots:
            if value == path or value.startswith(path + os.sep):
                return token + value[len(path):]
        return value

    return [{"name": row["name"], "arguments": [replace(a) for a in row["arguments"]],
             "cwd": replace(row.get("cwd", str(root / "coax"))),
             "stdout_path": replace(row["stdout_path"]), "stderr_path": replace(row["stderr_path"])}
            for row in record["commands"]]


def verify_receipt(plan, path):
    prior = json.loads(path.read_text(encoding="utf-8"))
    if not prior.get("passed"):
        raise BootstrapError("prior receipt is not a passing reconstruction")
    prior_root = Path(prior["root"])
    prior_work = prior.get("output_directory")
    if prior_work is None:
        prior_work = str(Path(prior["commands"][0]["stdout_path"]).parent)
    current_commands = normalized_commands(plan, Path(plan["root"]), Path(plan["output_directory"]))
    old_commands = normalized_commands(prior, prior_root, Path(prior_work))
    errors = []
    if current_commands != old_commands:
        errors.append("normalized command plan differs")
    fields = ("normal_seed_compile_order", "normal_seed_link_order", "seed_sha256",
              "authored_source_sha256", "normal_bootstrap_recipe_sha256")
    for field in fields:
        if plan[field] != prior.get(field):
            errors.append(field + " differs")
    if "embedded_protocol_sources_sha256" in prior and plan["embedded_protocol_sources_sha256"] != prior["embedded_protocol_sources_sha256"]:
        errors.append("embedded protocol source literals differ")
    if errors:
        raise BootstrapError("receipt verification failed: " + "; ".join(errors))
    encoded = json.dumps(current_commands, sort_keys=True, separators=(",", ":")).encode()
    return {
        "passed": True, "prior_receipt": str(path.resolve()), "command_count": len(current_commands),
        "normalized_command_sha256": hashlib.sha256(encoded).hexdigest(),
        "compared": ["all normalized command arguments/order/cwd/stdout/stderr", *fields],
        "full_normal_fingerprint_recomputed": False,
        "scope": "Selected recorded source/seed/recipe identities and command plan only. This read-only comparison does not recompute the full compiler-input/toolchain/SDK fingerprint or requalify a binary.",
        "prior_input_fingerprint": prior.get("input_fingerprint_before"),
        "source_commit_labels": {"current": plan["source_commit"], "prior": prior.get("source_commit")},
    }


def write_record(work, record):
    temporary = work / "receipt.json.tmp"
    temporary.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    temporary.replace(work / "receipt.json")


def stop_process(process):
    if process is None:
        return
    try:
        if os.name == "posix":
            # The driver may already have exited while its compiler children
            # remain in the new session's process group.
            os.killpg(process.pid, signal.SIGKILL)
        elif process.poll() is None:
            process.kill()
    except ProcessLookupError:
        pass
    process.wait(timeout=5)


def execute(plan, protocol, started):
    work = Path(plan["output_directory"])
    try:
        work.mkdir(parents=True, exist_ok=False)
    except FileExistsError as error:
        raise BootstrapError("output directory must be new; existing output is never overwritten") from error
    record = {key: value for key, value in plan.items() if key != "commands"}
    record.update(schema="coax.quantity.bootstrap.execution.v1", commands=[], passed=False)
    write_record(work, record)
    (work / "proto_runtime.inc").write_bytes(protocol)
    deadline = started + WALL_BUDGET_SECONDS
    environment = dict(os.environ)
    try:
        for command in plan["commands"]:
            remaining = deadline - time.monotonic()
            row = dict(command)
            row["remaining_wall_budget_seconds"] = max(0, remaining)
            command_started = time.monotonic()
            if remaining <= 0:
                row.update(exit=124, elapsed_seconds=0, timed_out=True)
                record["commands"].append(row)
                raise BootstrapError("shared 840-second reconstruction budget exhausted")
            process = None
            with open(row["stdout_path"], "wb") as stdout, open(row["stderr_path"], "wb") as stderr:
                try:
                    process = subprocess.Popen(row["arguments"], cwd=row["cwd"], env=environment,
                                               stdout=stdout, stderr=stderr, start_new_session=(os.name == "posix"))
                    code = process.wait(timeout=remaining)
                except subprocess.TimeoutExpired:
                    stop_process(process)
                    code = 124
                    row["timed_out"] = True
                except OSError as error:
                    stop_process(process)
                    stderr.write((str(error) + "\n").encode())
                    code = 127
                except BaseException as error:
                    stop_process(process)
                    row.update(exit=130 if isinstance(error, KeyboardInterrupt) else 1,
                               elapsed_seconds=time.monotonic() - command_started)
                    record["commands"].append(row)
                    raise
            row.update(exit=code, elapsed_seconds=time.monotonic() - command_started)
            record["commands"].append(row)
            write_record(work, record)
            print(f"{row['name']}: {code}", file=sys.stderr, flush=True)
            if code != 0:
                raise BootstrapError(f"{row['name']} failed; partial output and receipt remain in {work}")
        record["input_fingerprint_before"] = (work / "fingerprint-before.stdout").read_text().strip()
        record["input_fingerprint_after"] = (work / "fingerprint-after.stdout").read_text().strip()
        if record["input_fingerprint_before"] != record["input_fingerprint_after"]:
            raise BootstrapError("compiler source inputs changed during reconstruction")
        record["compiler_version"] = (work / "final-version.stdout").read_text().strip()
        record["artifacts"] = {name: sha(work / name) for name in ("stage0", "proto_importer.c", "proto_importer.h",
                                "proto_importer.o", "coax", "rebuilt.c", "rebuilt.h")}
        record["compiler_binary_path"] = str(work / "coax")
        record["fixed_points"] = {"compiler_graph_c": True, "compiler_graph_header": True,
                                  "checked_in_parser_seed_c_and_header": True, "checked_in_checker_seed_c_and_header": True}
        record["passed"] = True
    except BaseException as error:
        record["failure"] = str(error)
        raise
    finally:
        record["duration_seconds"] = time.monotonic() - started
        write_record(work, record)
    return record


def main(argv=None):
    started = time.monotonic()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("new_output_dir", type=Path, help="private output directory, which must not exist when executing")
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="source repository root (default: this script's repository)")
    parser.add_argument("--source-commit", help="full source commit label (otherwise read HEAD/ref files without invoking Git)")
    parser.add_argument("--plan", action="store_true", help="print the command plan and source/seed hashes without running subprocesses or writing files")
    parser.add_argument("--verify-receipt", type=Path, help="compare the normalized plan and selected identities with a prior passing JSON receipt")
    args = parser.parse_args(argv)
    try:
        root = args.root.resolve()
        work = private_output(root, args.new_output_dir, args.plan)
        commit = args.source_commit or commit_from_files(root)
        if commit is not None and not re.fullmatch(r"[0-9a-fA-F]{40}|[0-9a-fA-F]{64}", commit):
            raise BootstrapError("--source-commit must be a full hexadecimal commit ID")
        plan, protocol = plan_for(root, work, commit)
        if args.verify_receipt:
            plan["receipt_verification"] = verify_receipt(plan, args.verify_receipt)
        result = plan if args.plan else execute(plan, protocol, started)
        print(json.dumps(result, indent=2))
        return 0
    except KeyboardInterrupt:
        print("quantity bootstrap: interrupted; partial private output is retained", file=sys.stderr)
        return 130
    except (BootstrapError, OSError, ValueError, KeyError) as error:
        print("quantity bootstrap: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
