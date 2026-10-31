#!/usr/bin/env python3
"""Optional original-source native-check execution with Coax-owned policy.

No check bodies are read or rewritten by this tool. The compiler emits one
checked-AST C artifact; a fixed observer and Coax policy accompany it at O0/O2.
Python performs host effects and transports records/ABI values only.
"""
import argparse
import base64
import copy
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import tempfile
import time

import bounded_process as process
import native_execution_identities as identities
from native_execution_policy import Policy, PolicyUnavailable, OUTCOMES, ROOT, REPO, digest

SCHEMA = "coax.native-checks.execution.v1"
OBSERVER = ROOT / "tools/native-check-observer.c"
_POLICIES = {}


def _sha(path):
    try:
        return digest(path)
    except (OSError, ValueError):
        return ""


def _compiler(value=None):
    return Path(value or os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()


def _record(value):
    if type(value) is bytes:
        return {"base64": base64.b64encode(value).decode("ascii"),
                "bytes": len(value), "sha256": hashlib.sha256(value).hexdigest()}
    if type(value) is dict:
        return {str(k): _record(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [_record(v) for v in value]
    if isinstance(value, Path):
        return str(value)
    return value


def _identity(item):
    return {**{k: item[k] for k in ("ordinal", "declaration_index", "line", "column")},
            "name": item["name"].decode("utf-8", "backslashreplace"),
            "source": item["source"].decode("utf-8", "backslashreplace"),
            "name_base64": base64.b64encode(item["name"]).decode("ascii"),
            "source_base64": base64.b64encode(item["source"]).decode("ascii")}


def _last_json(observation):
    try:
        def invalid_constant(value):
            raise ValueError("non-JSON numeric constant: " + value)
        return json.loads(observation["stderr"].splitlines()[-1], parse_constant=invalid_constant)
    except (ValueError, IndexError, UnicodeError, TypeError):
        return None


def _baseline_records(observation):
    def invalid_constant(value):
        raise ValueError("non-JSON numeric constant: " + value)

    def records(stream):
        result = []
        for line in stream.splitlines():
            try:
                value = json.loads(line, parse_constant=invalid_constant)
                if type(value) is dict:
                    result.append(value)
            except (ValueError, UnicodeError):
                pass
        return result

    evidence = [value for value in records(observation["stderr"]) if value.get("schema") == "coax.evidence.v1"]
    manifests = [value for value in records(observation["stdout"]) if "schema_version" in value]
    return (evidence[0] if len(evidence) == 1 else None,
            manifests[0] if len(manifests) == 1 else None)


class Session:
    def __init__(self, deadline, environment=None):
        self.deadline = deadline
        self.environment = environment
        self.remaining_bytes = 67108864
        self.observations = []

    def run(self, argv, timeout_s=5.0, max_output_bytes=1048576, quiet_faults=True):
        cap = min(max_output_bytes, max(0, self.remaining_bytes))
        result = process.run(argv, REPO, self.environment, timeout_s, cap,
                             self.deadline, quiet_faults=quiet_faults)
        self.remaining_bytes -= result["observed_stdout_bytes"] + result["observed_stderr_bytes"]
        self.observations.append(result)
        return result


def _retained_policy(cached, directory):
    # The loaded library and retained inputs must still name the same bytes.
    # In particular, do not bless changed cached C while using the old DLL.
    for name in ('c', 'header', 'library'):
        if cached.integrity(cached.identity[name + '_sha256'], _sha(getattr(cached, name))):
            raise PolicyUnavailable('Retained execution policy artifact changed: ' + name, cached.observations)
    if directory is None:
        return cached
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=False)
    result = copy.copy(cached)
    result.directory, result.identity = directory, dict(cached.identity)
    for name in ('c', 'header', 'library'):
        destination = directory / getattr(cached, name).name
        shutil.copyfile(getattr(cached, name), destination)
        setattr(result, name, destination)
        result.identity[name] = str(destination)
        if cached.integrity(cached.identity[name + '_sha256'], _sha(destination)):
            raise PolicyUnavailable('Copied execution policy artifact changed: ' + name, cached.observations)
    result.identity['retained_bootstrap'] = dict(cached.identity)
    return result


def get_policy(compiler=None, directory=None, deadline=None, _runner=None):
    compiler = _compiler(compiler)
    deadline = deadline if deadline is not None else time.monotonic() + 840
    inputs = [ROOT / "lib/compiler" / name for name in (
        "native-execution.coax", "native-checks.coax", "mutation-schedule.coax", "budget.coax")]
    key = (_sha(compiler), tuple(_sha(p) for p in inputs),
           os.environ.get("COAX_EXECUTION_HOST_CC"), os.environ.get("CC"), os.environ.get("SDKROOT"))
    cached = _POLICIES.get(key)
    if cached is None:
        cache = Path(tempfile.mkdtemp(prefix="coax-execution-policy-")) / "policy"
        cached = Policy(compiler, cache, deadline, runner=_runner or Session(deadline).run)
        _POLICIES[key] = cached
    return _retained_policy(cached, directory)


def get_forall_policy(compiler=None, directory=None, deadline=None):
    from native_forall_execution import get_policy as get_quantified_policy
    return get_quantified_policy(compiler, directory, deadline)


def observe_forall_artifact(binary, checks, *, timeout_s=10.0, max_output_bytes=1048576,
                            deadline=None, policy=None):
    from native_forall_execution import observe_artifact as observe_quantified
    return observe_quantified(binary, checks, timeout_s=timeout_s, max_output_bytes=max_output_bytes,
                              deadline=deadline, policy=policy)


def classify_observation(expected_check, process_observation, *, policy=None):
    policy = policy or get_policy()
    code = policy.execution(expected_check, process_observation.get("stdout", b""), process_observation)
    return {"outcome": OUTCOMES[code], "outcome_code": code,
            "recognized_trap": False, "identity": _identity(expected_check),
            "observation": _record(process_observation)}


def _observe_artifact(binary, expected_checks, session, policy, timeout_s, max_output_bytes):
    checks = []
    original = _sha(binary)
    for expected in expected_checks:
        observation = session.run([str(binary), "one", str(expected["ordinal"])],
                                  timeout_s=timeout_s, max_output_bytes=max_output_bytes)
        entry = classify_observation(expected, observation, policy=policy)
        integrity = policy.integrity(original, _sha(binary))
        entry["integrity_outcome"] = OUTCOMES[integrity]
        entry["outcome"] = OUTCOMES[policy.aggregate([entry["outcome_code"], integrity])]
        checks.append(entry)
    code = policy.aggregate([item["outcome"] for item in checks])
    return {"outcome": OUTCOMES[code], "checks": checks,
            "scope": "Artifact observations only; no evaluator baseline or qualification is invented."}


def observe_artifact(binary, expected_checks, *, timeout_s=10.0,
                     max_output_bytes=1048576, deadline=None, policy=None):
    deadline = deadline if deadline is not None else time.monotonic() + 840
    policy = policy or get_policy(deadline=deadline)
    if not policy.budget(timeout_s, max_output_bytes):
        raise ValueError("execution bounds were refused by Coax policy")
    return _observe_artifact(Path(binary).resolve(), expected_checks, Session(deadline),
                             policy, timeout_s, max_output_bytes)


def _coverage(baseline, selection):
    claims = baseline.get("claims", []) if type(baseline) is dict else []
    labels = ("not-selected", "selected-test", "selected-forall")
    result = []
    for i, mask in enumerate(selection["mask"]):
        claim = claims[i]
        row = {"claim_index": i, "kind": claim.get("kind"), "name": claim.get("name"),
               "at": claim.get("at"), "selection": "evaluator-only" if claim.get("kind") == "#authority" else labels[mask]}
        result.append(row)
    return result


def _toolchain_outcomes(policy, chain):
    observations = [chain["version_observation"], chain["target_observation"]]
    sdk = chain["sdk"]
    for name in ("path_observation", "version_observation"):
        observation = sdk.get(name)
        if sdk.get("platform") == "Darwin" or observation is not None:
            observations.append(observation)
    return [policy.phase(value) for value in observations] + [
        policy.integrity(chain.get("program_sha256") or "", _sha(chain.get("resolved_program") or ""))]


def _toolchain_integrity(policy, chain):
    codes = [policy.integrity(chain.get('program_sha256') or '',
                              _sha(chain.get('resolved_program') or ''))]
    for fact in chain.get('sdk', {}).get('settings_files', []):
        codes.append(policy.integrity(fact.get('sha256') or '',
            _sha(fact.get('resolved_path') or fact.get('path') or '')))
    return codes


def run(files, *, keep=None, steps=None, only=None, timeout_s=10.0,
        max_output_bytes=1048576, cc=None, compiler=None, abi=1):
    if type(abi) is not int or abi != 1:
        from native_forall_execution import run as run_quantified
        return run_quantified(files, keep=keep, steps=steps, only=only, timeout_s=timeout_s,
                              max_output_bytes=max_output_bytes, cc=cc, compiler=compiler, abi=abi)
    started = time.monotonic()
    deadline = started + 840
    compiler = _compiler(compiler)
    work = Path(keep) if keep is not None else Path(tempfile.mkdtemp(prefix="coax-native-execution-"))
    if keep is not None:
        work.mkdir(parents=True, exist_ok=False)
    work = work.resolve()
    session = Session(deadline)
    result = {"schema": SCHEMA, "scope": "Complete original evaluator baseline and requested native selection only",
              "selection": {"only": only, "baseline_only": None, "baseline_list": False},
              "compiler": str(compiler), "compiler_sha256": _sha(compiler),
              "artifacts": str(work), "files": [], "outcome": "infrastructure-error"}
    try:
        policy = get_policy(compiler, work / "policy", deadline, _runner=session.run)
        result["policy"] = policy.identity
        result["policy_bootstrap"] = _record(policy.observations)
        if not policy.budget(timeout_s, max_output_bytes):
            result["why"] = "execution bounds were refused by Coax policy"
            return result
        version = session.run([str(compiler), "version"])
        result["compiler_version_execution"] = _record(version)
        version_outcome = policy.phase(version)
        if version_outcome != 0:
            result["outcome"] = OUTCOMES[version_outcome]
            return result
        compiler_version = version["stdout"].decode("utf-8", "strict").strip()
        result["compiler_version"] = compiler_version
        requested_steps = policy.limits.steps if steps is None else steps
        if not policy.step_request(requested_steps):
            result["why"] = "original evaluator step budget was refused by Coax policy before execution"
            return result
        native_cc = list(cc) if isinstance(cc, (list, tuple)) else shlex.split(cc or os.environ.get("COAX_NATIVE_CC", "cc"))
        result["native_cc"] = native_cc
        chain, chain_codes = None, []
        for file_index, filename in enumerate(files):
            source = Path(filename).resolve()
            directory = work / ("file-" + str(file_index))
            directory.mkdir()
            source_sha = _sha(source)
            item = {"file": str(source), "source_sha256": source_sha, "baseline": None,
                    "native_coverage": [], "profiles": {}, "outcome": "not-checked"}
            result["files"].append(item)
            baseline_argv = [str(compiler), "test", str(source), "--library", "--arithmetic", "checked",
                             "--evidence", "/dev/stderr", "--profile-report", "/dev/stdout",
                             "--steps", str(requested_steps)]
            baseline_observation = session.run(baseline_argv, timeout_s=policy.limits.baseline_ms / 1000,
                                               max_output_bytes=policy.limits.baseline_output_bytes)
            baseline, baseline_manifest = _baseline_records(baseline_observation)
            item["baseline"], item["baseline_execution"] = baseline, _record(baseline_observation)
            item["baseline_input_manifest"] = baseline_manifest
            validation = policy.baseline(baseline, baseline_observation, source, source_sha,
                                         result["compiler_sha256"], compiler_version, requested_steps)
            item["baseline_validation"] = validation
            if validation["outcome"] != "passed":
                item["outcome"] = validation["outcome"]
                continue
            baseline_inputs = policy.baseline_inputs(baseline, baseline_manifest, baseline_observation, compiler_version)
            item["baseline_input_validation"] = OUTCOMES[baseline_inputs]
            if baseline_inputs != 0:
                item["outcome"] = OUTCOMES[baseline_inputs]
                continue
            selection = policy.select(baseline, only)
            item["native_coverage"] = _coverage(baseline, selection)
            item["native_selection"] = {k: v for k, v in selection.items() if k != "identities"}
            item["checks"] = [_identity(value) for value in selection["identities"]]
            if selection["outcome"] != "passed":
                item["outcome"] = selection["outcome"]
                continue
            expected = selection["identities"]
            emission_argv = [str(compiler), "checks-c", str(source), "--arithmetic", "checked",
                             "--profile-report", "/dev/stderr"]
            if only is not None:
                emission_argv += ["--only", only]
            emission = session.run(emission_argv, timeout_s=policy.limits.compile_ms / 1000,
                                   max_output_bytes=policy.limits.baseline_output_bytes)
            manifest = _last_json(emission)
            c_file = directory / "checks.c"
            c_file.write_bytes(emission["stdout"])
            c_sha = digest(c_file)
            emission_code = policy.aggregate([policy.emission(manifest, baseline, emission, compiler_version, c_sha),
                                               policy.input_descriptors(baseline_manifest, manifest)])
            item["emission_execution"], item["emission_manifest"] = _record(emission), manifest
            item["artifact"] = {"c": str(c_file), "c_sha256": c_sha,
                                "observer": str(OBSERVER), "observer_sha256": digest(OBSERVER),
                                "observer_header": str(OBSERVER.with_suffix(".h")),
                                "observer_header_sha256": digest(OBSERVER.with_suffix(".h")),
                                "policy_c": str(policy.c), "policy_c_sha256": digest(policy.c),
                                "policy_header": str(policy.header), "policy_header_sha256": digest(policy.header)}
            codes = [emission_code, policy.integrity(result["compiler_sha256"], _sha(compiler))]
            if policy.aggregate(codes) != 0:
                item["outcome"] = OUTCOMES[policy.aggregate(codes)]
                continue
            if chain is None:
                chain = identities.inspect(native_cc, session.run, cwd=REPO, environment=session.environment)
                chain_codes = _toolchain_outcomes(policy, chain)
                result["toolchain"] = _record(chain)
                if policy.aggregate(chain_codes) == 0 and chain["sdk"].get("environment"):
                    session.environment = {**os.environ, **chain["sdk"]["environment"]}
            codes += chain_codes
            if policy.aggregate(codes) != 0:
                item["outcome"] = OUTCOMES[policy.aggregate(codes)]
                continue
            for optimization in ("-O0", "-O2"):
                binary = directory / ("native-" + optimization[1:])
                profile = {"binary": str(binary), "binary_sha256": "", "outcome": "not-checked"}
                item["profiles"][optimization] = profile
                build = session.run([*native_cc, "-std=c11", optimization, "-ffp-contract=off", "-fno-fast-math",
                                     "-I", str(policy.directory), str(c_file), str(policy.c), str(OBSERVER),
                                     "-lm", "-o", str(binary)],
                                    timeout_s=policy.limits.compile_ms / 1000,
                                    max_output_bytes=max_output_bytes)
                profile["compile"] = _record(build)
                compile_code = policy.phase(build)
                profile["binary_sha256"] = _sha(binary)
                profile_codes = [compile_code, policy.integrity(c_sha, _sha(c_file)),
                                 policy.integrity(item["artifact"]["observer_sha256"], _sha(OBSERVER)),
                                 policy.integrity(item["artifact"]["observer_header_sha256"], _sha(OBSERVER.with_suffix(".h"))),
                                 policy.integrity(item["artifact"]["policy_c_sha256"], _sha(policy.c)),
                                 policy.integrity(item["artifact"]["policy_header_sha256"], _sha(policy.header)),
                                 policy.integrity(profile["binary_sha256"], _sha(binary))]
                if policy.aggregate(profile_codes) == 0:
                    metadata = session.run([str(binary), "metadata"],
                                           timeout_s=policy.limits.metadata_ms / 1000,
                                           max_output_bytes=policy.limits.metadata_bytes)
                    profile["metadata"] = _record(metadata)
                    metadata_code = policy.metadata(expected, metadata["stdout"], metadata)
                    profile_codes.append(metadata_code)
                    if metadata_code == 0:
                        profile["catalog"] = [_identity(value) for value in policy.decode_catalog(metadata["stdout"])]
                        observed = _observe_artifact(binary, expected, session, policy, timeout_s, max_output_bytes)
                        profile["execution"] = observed
                        profile_codes.append(OUTCOMES.index(observed["outcome"]))
                profile["outcome"] = OUTCOMES[policy.aggregate(profile_codes)]
                codes.append(OUTCOMES.index(profile["outcome"]))
            final_sources = policy.baseline(baseline, baseline_observation, source, source_sha,
                                           result["compiler_sha256"], compiler_version, requested_steps)
            item["final_source_validation"] = final_sources
            codes += [OUTCOMES.index(final_sources["outcome"]),
                      policy.baseline_inputs(baseline, baseline_manifest, baseline_observation, compiler_version),
                      policy.input_descriptors(baseline_manifest, manifest),
                      policy.integrity(result["compiler_sha256"], _sha(compiler)),
                      policy.integrity(policy.identity["source_sha256"], _sha(policy.source))]
            codes += [policy.integrity(expected_sha, _sha(path)) for path, expected_sha in policy.identity["sources"].items()]
            codes.append(policy.integrity(policy.identity["library_sha256"], _sha(policy.library)))
            codes += _toolchain_integrity(policy, chain)
            item["outcome"] = OUTCOMES[policy.aggregate(codes)]
        result["outcome"] = OUTCOMES[policy.aggregate([item["outcome"] for item in result["files"]])]
    except PolicyUnavailable as error:
        result["why"] = str(error)
        result["policy_bootstrap"] = _record(error.observations)
        result["policy_loaded"] = False
    except (OSError, ValueError, UnicodeError) as error:
        result["why"] = str(error)
        result["startup_or_transport_error"] = type(error).__name__
    finally:
        result["elapsed_s"] = time.monotonic() - started
        result["compiler_sha256_after"] = _sha(compiler)
    return result


def _write_report(path, record, sources):
    path = Path(path)
    canonical = path.resolve()
    for source in sources:
        source = Path(source)
        if canonical == source.resolve() or (path.exists() and source.exists() and path.samefile(source)):
            raise ValueError("execution report aliases a consumed source or tool")
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + "." + str(os.getpid()) + ".tmp")
    created = False
    try:
        with temporary.open("x") as stream:
            created = True
            json.dump(record, stream, indent=2, ensure_ascii=False)
            stream.write("\n")
        temporary.replace(path)
    finally:
        if created and temporary.exists():
            temporary.unlink()


def write_report(path, record, compiler=None):
    consumed = [compiler or _compiler(), __file__, OBSERVER, OBSERVER.with_suffix(".h"),
                ROOT / "tools/native_execution_policy.py", ROOT / "tools/bounded_process.py",
                ROOT / "tools/native_execution_identities.py", ROOT / "tests/quiet_crash.py"]

    def paths(facts, names):
        if type(facts) is dict:
            consumed.extend(facts[name] for name in names if facts.get(name))

    def inputs(manifest):
        if type(manifest) is dict:
            for name in ("sources", "descriptors"):
                for fact in manifest.get(name, []):
                    paths(fact, ("path", "resolved_path"))

    def arguments(value, inherited_cwd=None):
        cwd = Path(REPO if inherited_cwd is None else inherited_cwd)
        if type(value) is dict:
            if type(value.get("cwd")) is str:
                recorded_cwd = Path(value["cwd"])
                cwd = recorded_cwd if recorded_cwd.is_absolute() else cwd / recorded_cwd
            for name, child in value.items():
                if name in ("argv", "launched_argv", "host_cc", "native_cc") and type(child) is list:
                    argument_cwd = Path(REPO) if name in ("host_cc", "native_cc") else cwd
                    for argument in child:
                        if type(argument) is str:
                            candidate = Path(argument)
                            if not candidate.is_absolute():
                                candidate = argument_cwd / candidate
                            if candidate.is_file():
                                consumed.append(candidate)
                            if not os.path.dirname(argument):
                                program = shutil.which(argument)
                                if program and Path(program).is_file():
                                    consumed.append(program)
                else:
                    arguments(child, cwd)
        elif type(value) is list:
            for child in value:
                arguments(child, cwd)

    arguments(record)
    consumed += list(record.get("transport_sources", {}))

    toolchain = record.get("toolchain", {})
    paths(toolchain, ("resolved_program",))
    sdk = toolchain.get("sdk", {})
    for fact in sdk.get("settings_files", []):
        paths(fact, ("path", "resolved_path"))
    if type(record.get("policy")) is dict:
        policy = record["policy"]
        consumed += list(policy.get("sources", {}))
        paths(policy, ("source", "c", "header", "library"))
        paths(policy.get("retained_bootstrap"), ("c", "header", "library"))
        inputs(policy.get("emission_manifest"))
    for item in record["files"]:
        consumed.append(item["file"])
        artifact = item.get("artifact", {})
        consumed += [artifact[name] for name in ("c", "observer", "observer_header", "policy_c", "policy_header") if name in artifact]
        consumed += [profile["binary"] for profile in item.get("profiles", {}).values() if "binary" in profile]
        if type(item.get("baseline")) is dict:
            consumed += [source["path"] for source in item["baseline"].get("sources", [])]
        inputs(item.get("emission_manifest"))
        inputs(item.get("baseline_input_manifest"))
    _write_report(path, record, consumed)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+")
    parser.add_argument("--keep")
    parser.add_argument("--steps", type=int)
    parser.add_argument("--only")
    parser.add_argument("--timeout", type=float, default=10.0)
    parser.add_argument("--max-output-bytes", type=int, default=1048576)
    parser.add_argument("--cc")
    parser.add_argument("--compiler")
    parser.add_argument("--verification-abi", type=int, default=1)
    parser.add_argument("--out")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    result = run(args.files, keep=args.keep, steps=args.steps, only=args.only, timeout_s=args.timeout,
                 max_output_bytes=args.max_output_bytes, cc=args.cc, compiler=args.compiler,
                 abi=args.verification_abi)
    if args.out:
        write_report(args.out, result, compiler=args.compiler)
    if args.json:
        print(json.dumps(result, indent=2, ensure_ascii=False))
    else:
        print(result["outcome"] + ": " + str(len(result["files"])) + " original source files")
    return 0 if result["outcome"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
