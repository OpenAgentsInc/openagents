"""Native Coax policy ABI adapter; this module transports facts, not verdict rules."""
import ctypes as C
import hashlib
import json
from functools import lru_cache
import math
import os
from pathlib import Path
import shlex
import stat
import sys
import time

import bounded_process as process

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
OUTCOMES = ("passed", "counterexample", "inconclusive", "infrastructure-error", "not-checked")
I, U, B = C.c_int64, C.c_uint32, C.c_bool


class String(C.Structure):
    _fields_ = [("data", C.c_void_p), ("count", I)]


class Identity(C.Structure):
    _fields_ = [("ordinal", I), ("declaration_index", I), ("line", I),
                ("column", I), ("name", String), ("source", String)]


class Frame(C.Structure):
    _fields_ = [("ok", B), ("kind", U), *[(n, I) for n in (
        "next", "count", "ordinal", "declaration_index", "line", "column",
        "name_offset", "name_count", "source_offset", "source_count")]]


class Process(C.Structure):
    _fields_ = [("launched", B), ("finished", B), ("timeout", B),
                ("output_limit", B), ("returncode", I), ("read_errno", I),
                ("stdout_eof", B), ("stderr_eof", B)]


class Base(C.Structure):
    _fields_ = [("shape", U), *[(n, String) for n in (
        "schema", "command", "input", "compiler_sha", "compiler_version", "target",
        "arithmetic", "runtime_arithmetic", "front_end", "verdict")],
        ("library", I), ("only_null", B), *[(n, I) for n in (
        "listing", "root_only", "exit_status", "check_steps", "slots", "containers", "run_steps")]]


class Source(C.Structure):
    _fields_ = [(n, String) for n in ("path", "observed_path", "sha", "observed_sha")]


class Claim(C.Structure):
    _fields_ = [(n, String) for n in ("kind", "name", "source", "outcome")] + [
        ("name_string", B), ("at_null", B), ("line", I), ("column", I),
        ("cases", I), ("steps", I)]


class BaseResult(C.Structure):
    _fields_ = [("valid", B), ("outcome", U), ("reason", U)]


class Plan(C.Structure):
    _fields_ = [("outcome", U), ("count", I)]


class Limits(C.Structure):
    _fields_ = [(n, I) for n in (
        "checks", "text_bytes", "frame_bytes", "metadata_bytes", "process_output_bytes",
        "baseline_output_bytes", "compile_ms", "case_ms", "metadata_ms", "baseline_ms", "total_ms", "steps")]


class Emission(C.Structure):
    _fields_ = [("schema", I), *[(n, String) for n in (
        "command", "backend", "compiler", "runtime_arithmetic", "interpreter_arithmetic", "build_profile")],
        ("success", I), ("exit_status", I), ("emitted_sha", String)]


@lru_cache(maxsize=None)
def view_type(element):
    class View(C.Structure):
        _fields_ = [("data", C.POINTER(element)), ("count", I)]
    return View


Bytes, Sources, Claims, Identities = map(view_type, (C.c_uint8, Source, Claim, Identity))


def digest(path):
    path = Path(path)
    metadata = path.stat()
    if not stat.S_ISREG(metadata.st_mode):
        raise OSError("identity input is not a regular file")
    result = hashlib.sha256()
    expires = time.monotonic() + 5
    remaining = metadata.st_size
    with path.open("rb") as stream:
        while remaining:
            if time.monotonic() >= expires:
                raise OSError("identity input exceeded its read deadline")
            block = stream.read(min(1048576, remaining))
            if not block:
                raise OSError("identity input shortened while reading")
            result.update(block)
            remaining -= len(block)
    return result.hexdigest()


def integer(value):
    return value if type(value) is int and -(1 << 63) <= value < (1 << 63) else -1


def boolean(value):
    return int(value) if type(value) is bool else -1


def text(value):
    if type(value) is not str:
        return b""
    try:
        return value.encode("utf-8")
    except UnicodeError:
        return b""


def string_observed(value):
    if type(value) is not str:
        return False
    try:
        value.encode("utf-8")
        return True
    except UnicodeError:
        return False


def object_value(value):
    return value if type(value) is dict else {}


class Arena:
    def __init__(self):
        self.keep = []

    def string(self, value):
        value = value if type(value) is bytes else text(value)
        buffer = C.create_string_buffer(value)
        self.keep.append(buffer)
        return String(C.cast(buffer, C.c_void_p), len(value))

    def array(self, element, values):
        array = (element * len(values))(*values)
        self.keep.append(array)
        return view_type(element)(C.cast(array, C.POINTER(element)), len(values))

    def bytes(self, value):
        array = (C.c_uint8 * len(value)).from_buffer_copy(value)
        self.keep.append(array)
        return Bytes(C.cast(array, C.POINTER(C.c_uint8)), len(value))

    def identity(self, item):
        return Identity(*[integer(item.get(n)) for n in ("ordinal", "declaration_index", "line", "column")],
                        self.string(item.get("name", b"")), self.string(item.get("source", b"")))


def process_fact(observation):
    observation = object_value(observation)
    code = observation.get("returncode")
    fields = ("started", "timeout", "deadline_exhausted", "output_limit", "stdout_limit", "stderr_limit", "stdout_eof", "stderr_eof")
    shape = all(type(observation.get(name)) is bool for name in fields)
    read_errno = 0 if "read_errno" in observation and observation["read_errno"] is None else integer(observation.get("read_errno"))
    return Process(observation.get("started") is True, type(code) is int,
                   observation.get("timeout") is True or observation.get("deadline_exhausted") is True,
                   any(observation.get(n) is True for n in ("output_limit", "stdout_limit", "stderr_limit")),
                   integer(code), read_errno if shape else -1,
                   observation.get("stdout_eof") is True, observation.get("stderr_eof") is True)


def copy_identity(value):
    return {"ordinal": value.ordinal, "declaration_index": value.declaration_index,
            "line": value.line, "column": value.column,
            "name": C.string_at(value.name.data, value.name.count),
            "source": C.string_at(value.source.data, value.source.count)}


class PolicyUnavailable(RuntimeError):
    def __init__(self, message, observations):
        super().__init__(message)
        self.observations = observations


class Policy:
    source_name = "native-execution.coax"
    file_stem = "native-check-execution-policy"
    input_names = ("native-execution.coax", "native-checks.coax", "mutation-schedule.coax", "budget.coax")

    def __init__(self, compiler, directory, deadline, runner=None):
        self.directory = Path(directory)
        self.directory.mkdir(parents=True, exist_ok=False)
        self.c = self.directory / (self.file_stem + ".c")
        self.header = self.c.with_suffix(".h")
        self.library = self.directory / ("native-check-execution-policy.dylib" if sys.platform == "darwin" else "native-check-execution-policy.so")
        self.compiler = Path(compiler).resolve()
        self.observations = []
        self.source = ROOT / "lib/compiler" / self.source_name
        def invoke(argv, seconds, cap):
            if runner is not None:
                return runner(argv, timeout_s=seconds, max_output_bytes=cap, quiet_faults=True)
            return process.run(argv, REPO, None, seconds, cap, deadline, quiet_faults=True)
        source_inputs = {str(ROOT / "lib/compiler" / name): digest(ROOT / "lib/compiler" / name)
                         for name in self.input_names}
        request = [str(self.compiler), "c", str(self.source), "--library", "--header", str(self.header),
                   "--profile-report", "/dev/stderr"]
        result = invoke(request, 120, 67108864)
        self.observations.append(result)
        if result["returncode"] != 0 or any(result[n] for n in ("timeout", "deadline_exhausted", "output_limit", "stdout_limit", "stderr_limit", "read_errno")) or not (result["stdout_eof"] and result["stderr_eof"]):
            raise PolicyUnavailable("Coax execution policy could not be emitted", self.observations)
        self.c.write_bytes(result["stdout"])
        try:
            source_manifest = json.loads(result["stderr"].splitlines()[-1])
        except (ValueError, IndexError, UnicodeError) as error:
            raise PolicyUnavailable("execution policy emission has no source manifest", self.observations) from error
        host_cc = shlex.split(os.environ.get("COAX_EXECUTION_HOST_CC", os.environ.get("CC", "cc")))
        options = ["-dynamiclib"] if sys.platform == "darwin" else ["-shared", "-fPIC"]
        command = [*host_cc, "-std=c11", "-O1", "-ffp-contract=off", "-fno-fast-math", *options,
                   str(self.c), "-lm", "-o", str(self.library)]
        result = invoke(command, 120, 1048576)
        self.observations.append(result)
        if result["returncode"] != 0 or any(result[n] for n in ("timeout", "deadline_exhausted", "output_limit", "stdout_limit", "stderr_limit", "read_errno")) or not (result["stdout_eof"] and result["stderr_eof"]):
            raise PolicyUnavailable("native execution policy library could not be compiled", self.observations)
        try:
            self.dll = C.CDLL(str(self.library))
            self._bind("EP_limits", [], Limits)
            self._bind("EP_budget", [I, I, I], B)
            self._bind("EP_steps", [I], B)
            self._bind("EP_phase", [Process], U)
            self._bind("EP_decode", [Bytes, I], Frame)
            self._bind("EP_metadata", [Identities, Bytes, Process], U)
            self._bind("EP_execution", [Identity, Bytes, Process], U)
            self._bind("EP_baseline", [Base, Sources, Claims, Process, String, String, String, String, I], BaseResult)
            self._bind("EP_plan", [Claims, String, B, Bytes], Plan)
            self._bind("EP_selected", [Claims, Bytes, I], Identity)
            self._bind("EP_emission", [Emission, Sources, Sources, Process, String, String], U)
            self._bind("EP_baseline_inputs", [Emission, Sources, Sources, Sources, B, B, Process, String], U)
            self._bind("EP_input_descriptors", [Sources, Sources, B, B], U)
            self._bind("EP_sources_match", [Sources, Sources], B)
            self._bind("EP_integrity", [String, String], U)
            self._bind("EV_add", [U, U], U)
            self._bind("EV_verdict", [U], U)
        except (OSError, AttributeError) as error:
            raise PolicyUnavailable(str(error), self.observations) from error
        self.limits = self.dll.EP_limits()
        arena = Arena()
        expected_sources = [{"path": path, "sha256": value} for path, value in source_inputs.items()]
        if not self.dll.EP_sources_match(self._sources(arena, expected_sources),
                                        self._sources(arena, object_value(source_manifest).get("sources"))) or self.integrity(
                                            object_value(source_manifest).get("emitted_sha256"), digest(self.c)) != 0:
            raise PolicyUnavailable("execution policy source/C identities changed during bootstrap", self.observations)
        self.identity = {"source": str(self.source), "source_sha256": digest(self.source),
                         "c": str(self.c), "c_sha256": digest(self.c), "header": str(self.header),
                         "header_sha256": digest(self.header), "library": str(self.library),
                         "library_sha256": digest(self.library), "host_cc": host_cc}
        self.identity["sources"] = source_inputs
        self.identity["emission_manifest"] = source_manifest

    def _bind(self, name, arguments, result):
        function = getattr(self.dll, name)
        function.argtypes, function.restype = arguments, result

    def aggregate(self, outcomes):
        bits = 0
        for outcome in outcomes:
            bits = self.dll.EV_add(bits, OUTCOMES.index(outcome) if isinstance(outcome, str) else outcome)
        return int(self.dll.EV_verdict(bits))

    def phase(self, observation):
        return int(self.dll.EP_phase(process_fact(observation)))

    def integrity(self, expected, observed):
        arena = Arena()
        return int(self.dll.EP_integrity(arena.string(expected), arena.string(observed)))

    def budget(self, timeout_s, output_bytes):
        milliseconds = -1
        if type(timeout_s) in (int, float) and math.isfinite(timeout_s):
            milliseconds = integer(math.ceil(timeout_s * 1000))
        return bool(self.dll.EP_budget(milliseconds, integer(output_bytes), self.limits.total_ms))

    def step_request(self, value):
        return bool(self.dll.EP_steps(integer(value)))

    def _sources(self, arena, values):
        result = []
        for item in values if type(values) is list else []:
            item = object_value(item)
            raw_path = item.get("path")
            observed_path, observed_sha = "", ""
            if string_observed(raw_path):
                try:
                    actual = Path(raw_path).resolve()
                    observed_path, observed_sha = str(actual), digest(actual)
                except (OSError, ValueError):
                    pass
            result.append(Source(arena.string(raw_path), arena.string(observed_path),
                                 arena.string(item.get("sha256")), arena.string(observed_sha)))
        return arena.array(Source, result)

    def _claims(self, arena, values):
        result = []
        for item in values if type(values) is list else []:
            item = object_value(item)
            at = object_value(item.get("at"))
            source = ""
            if string_observed(at.get("file")) and at["file"]:
                try:
                    source = str((REPO / at["file"]).resolve())
                except (OSError, ValueError):
                    pass
            result.append(Claim(*[arena.string(v) for v in (item.get("kind"), item.get("name"), source, item.get("outcome"))],
                                string_observed(item.get("name")), "at" in item and item["at"] is None,
                                integer(at.get("line")), integer(at.get("col")),
                                integer(item.get("cases")), integer(item.get("steps"))))
        return arena.array(Claim, result)

    def baseline(self, record, observation, input_path, input_sha, compiler_sha, compiler_version, steps):
        arena = Arena()
        value = object_value(record)
        compiler, target, profile, selection, budgets = [object_value(value.get(n)) for n in (
            "compiler", "target", "profile", "selection", "budgets")]
        shape_values = [type(record) is dict, *[type(value.get(n)) is dict for n in (
            "compiler", "target", "profile", "selection", "budgets")],
            type(value.get("sources")) is list, type(value.get("claims")) is list]
        shape = sum(int(present) << i for i, present in enumerate(shape_values))
        base = Base(shape, *[arena.string(v) for v in (
            value.get("schema"), value.get("command"), value.get("input"), compiler.get("sha256"),
            compiler.get("version"), target.get("name"), profile.get("arithmetic"),
            profile.get("selected_runtime_arithmetic"), value.get("front_end"), value.get("verdict"))],
            boolean(profile.get("library")), "only" in selection and selection["only"] is None,
            boolean(selection.get("list")), boolean(selection.get("root_only")), integer(value.get("exit_status")),
            *[integer(budgets.get(n)) for n in ("check_steps", "check_slots", "check_containers", "run_steps")])
        sources, claims = self._sources(arena, value.get("sources")), self._claims(arena, value.get("claims"))
        result = self.dll.EP_baseline(base, sources, claims, process_fact(observation),
                                      arena.string(str(input_path)), arena.string(input_sha),
                                      arena.string(compiler_sha), arena.string(compiler_version), integer(steps))
        return {"valid": bool(result.valid), "outcome": OUTCOMES[result.outcome], "reason": int(result.reason)}

    def select(self, record, only):
        arena = Arena()
        claims = self._claims(arena, object_value(record).get("claims"))
        mask_array = (C.c_uint8 * claims.count)()
        mask = Bytes(C.cast(mask_array, C.POINTER(C.c_uint8)), claims.count)
        result = self.dll.EP_plan(claims, arena.string(only), only is not None, mask)
        identities = [copy_identity(self.dll.EP_selected(claims, mask, i)) for i in range(result.count)] if result.outcome == 0 else []
        return {"outcome": OUTCOMES[result.outcome], "count": int(result.count), "mask": list(mask_array), "identities": identities}

    def emission(self, record, baseline, observation, compiler_version, c_sha):
        arena = Arena()
        value = object_value(record)
        item = Emission(integer(value.get("schema_version")), *[arena.string(value.get(n)) for n in (
            "command", "backend", "compiler", "runtime_arithmetic", "interpreter_arithmetic", "build_profile")],
            boolean(value.get("success")), integer(value.get("exit_status")), arena.string(value.get("emitted_sha256")))
        base_sources, emitted_sources = self._sources(arena, object_value(baseline).get("sources")), self._sources(arena, value.get("sources"))
        return int(self.dll.EP_emission(item, base_sources, emitted_sources, process_fact(observation),
                                        arena.string(compiler_version), arena.string(c_sha)))

    def baseline_inputs(self, baseline_record, baseline_manifest, observation, compiler_version):
        arena = Arena()
        baseline, manifest = object_value(baseline_record), object_value(baseline_manifest)
        item = Emission(integer(manifest.get("schema_version")), *[arena.string(manifest.get(n)) for n in (
            "command", "backend", "compiler", "runtime_arithmetic", "interpreter_arithmetic", "build_profile")],
            boolean(manifest.get("success")), integer(manifest.get("exit_status")), arena.string(manifest.get("emitted_sha256")))
        evidence_sources, profile_sources, descriptors = baseline.get("sources"), manifest.get("sources"), manifest.get("descriptors")
        return int(self.dll.EP_baseline_inputs(
            item, self._sources(arena, evidence_sources), self._sources(arena, profile_sources),
            self._sources(arena, descriptors), type(evidence_sources) is list and type(profile_sources) is list,
            type(descriptors) is list, process_fact(observation), arena.string(compiler_version)))

    def input_descriptors(self, baseline_manifest, emission_manifest):
        arena = Arena()
        baseline = object_value(baseline_manifest).get("descriptors")
        emitted = object_value(emission_manifest).get("descriptors")
        return int(self.dll.EP_input_descriptors(
            self._sources(arena, baseline), self._sources(arena, emitted),
            type(baseline) is list, type(emitted) is list))

    def metadata(self, expected, data, observation):
        arena = Arena()
        identities = arena.array(Identity, [arena.identity(item) for item in expected])
        return int(self.dll.EP_metadata(identities, arena.bytes(data), process_fact(observation)))

    def execution(self, expected, data, observation):
        arena = Arena()
        return int(self.dll.EP_execution(arena.identity(expected), arena.bytes(data), process_fact(observation)))

    def decode_catalog(self, data):
        arena = Arena()
        raw = arena.bytes(data)
        head = self.dll.EP_decode(raw, 0)
        result, offset = [], head.next
        for _ in range(head.count):
            row = self.dll.EP_decode(raw, offset)
            result.append({"ordinal": row.ordinal, "declaration_index": row.declaration_index,
                           "line": row.line, "column": row.column,
                           "name": data[row.name_offset:row.name_offset + row.name_count],
                           "source": data[row.source_offset:row.source_offset + row.source_count]})
            offset = row.next
        return result
