# verify.method_conformance: run one registry entry's property checks
# against one candidate function.
#
# The host passes one argument, a base64 JSON spec:
#   {"workdir": ..., "file": "pkg/mod.py", "qualname": "f" or "Class.method",
#    "call": {...}, "tolerance": 1e-6, "properties": [...]}
# and reads the one line that starts with MARK from standard output.
#
# Every check is code: a call with the property's inputs, and a
# comparison with the value, relation, or count the property states.
# Nothing here judges; nothing here reads the task.

import base64
import contextlib
import importlib
import importlib.util
import inspect
import io
import json
import math
import os
import signal
import sys

MARK = "CONFORMANCE-RESULT:"
CALL_SEC = 10
TEXT = 300

try:
    import numpy as _np
except Exception:  # numpy is optional; the plain form needs none
    _np = None


class Timeout(Exception):
    pass


def _alarm(_signum, _frame):
    raise Timeout(f"the call did not return within {CALL_SEC} s")


def clip(text):
    text = str(text)
    return text if len(text) <= TEXT else text[: TEXT - 3] + "..."


def describe(value):
    try:
        if _np is not None and isinstance(value, _np.ndarray):
            return clip(json.dumps(value.tolist()))
        if isinstance(value, float):
            return repr(value)
        return clip(json.dumps(value, default=repr))
    except Exception:
        return clip(repr(value))


def load(workdir, path):
    sys.path.insert(0, workdir)
    rel = path[:-3] if path.endswith(".py") else path
    parts = [p for p in rel.split("/") if p]
    if parts and parts[-1] == "__init__":
        parts = parts[:-1]
    name = ".".join(parts)
    first = None
    if name and all(p.isidentifier() for p in parts):
        try:
            return importlib.import_module(name)
        except BaseException as error:  # a script may exit on import
            first = error
    here = os.path.join(workdir, os.path.dirname(path))
    sys.path.insert(0, here)
    spec = importlib.util.spec_from_file_location("_conformance_candidate", os.path.join(workdir, path))
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load {path}: {first}")
    module = importlib.util.module_from_spec(spec)
    sys.modules["_conformance_candidate"] = module
    spec.loader.exec_module(module)
    return module


def numeric_list(value):
    return isinstance(value, list) and all(
        isinstance(x, (int, float)) and not isinstance(x, bool) or numeric_list(x) for x in value
    )


def convert(value, form):
    if form == "numpy":
        if _np is None:
            raise RuntimeError("numpy isn't installed")
        if numeric_list(value):
            return _np.asarray(value, dtype=float)
        if isinstance(value, list):
            return _np.asarray(value)
        return value
    if form == "rows":
        if _np is None:
            raise RuntimeError("numpy isn't installed")
        if numeric_list(value):
            array = _np.asarray(value, dtype=float)
            return array.reshape(1, -1) if array.ndim == 1 else array
        return value
    return value


class Target:
    """The candidate: a function, a static method, or a method of a class
    built with no arguments, or else with the entry's fill values."""

    def __init__(self, module, qualname):
        parts = qualname.split(".")
        self.owner = None
        self.name = parts[-1]
        if len(parts) == 1:
            self.function = getattr(module, parts[0])
        else:
            owner = module
            for part in parts[:-1]:
                owner = getattr(owner, part)
            self.owner = owner
            raw = inspect.getattr_static(owner, self.name)
            if isinstance(raw, (staticmethod, classmethod)):
                self.function = getattr(owner, self.name)
                self.owner = None
            else:
                self.function = None

    def instance(self, fill):
        try:
            return self.owner()
        except TypeError:
            if not fill:
                raise
            return self.owner(*fill)

    def bound(self, fill):
        if self.owner is None:
            return self.function
        return getattr(self.instance(fill), self.name)


def fill_args(function, args, fill):
    try:
        signature = inspect.signature(function)
    except (TypeError, ValueError):
        return args
    required = [
        p
        for p in signature.parameters.values()
        if p.kind in (p.POSITIONAL_ONLY, p.POSITIONAL_OR_KEYWORD) and p.default is p.empty
    ]
    missing = len(required) - len(args)
    if missing > 0 and missing <= len(fill):
        return list(args) + list(fill[:missing])
    return args


def invoke(target, args, form, fill):
    signal.alarm(CALL_SEC)
    try:
        if form == "stepwise":
            if target.owner is None:
                raise TypeError("the stepwise form needs a method of a class")
            method = getattr(target.instance(fill), target.name)
            out = []
            for sample in args[0]:
                out.append(method(sample))
            if all(x is None for x in out):
                raise TypeError("the method returned nothing for any sample")
            return out
        function = target.bound(fill)
        converted = [convert(a, form) for a in args]
        converted = fill_args(function, converted, fill)
        with contextlib.redirect_stdout(io.StringIO()):
            result = function(*converted)
        if form == "rows":
            result = unwrap_row(result)
        return result
    finally:
        signal.alarm(0)


def unwrap_row(result):
    if _np is not None and isinstance(result, _np.ndarray):
        if result.ndim >= 1 and result.shape[0] == 1:
            return result[0]
        return result
    if isinstance(result, (list, tuple)) and len(result) == 1:
        return result[0]
    return result


def scalar(value):
    if hasattr(value, "statistic"):
        value = value.statistic
    if isinstance(value, tuple) and len(value) >= 1:
        value = value[0]
    if _np is not None and isinstance(value, _np.ndarray):
        if value.size != 1:
            raise ValueError(f"expected one number, got an array of shape {value.shape}")
        value = value.reshape(-1)[0]
    if isinstance(value, list) and len(value) == 1:
        value = value[0]
    if hasattr(value, "item") and not isinstance(value, (int, float)):
        value = value.item()
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"expected a number, got {describe(value)}")
    return float(value)


def vector(value):
    if _np is not None and isinstance(value, _np.ndarray):
        value = value.reshape(-1).tolist()
    elif hasattr(value, "tolist"):
        value = value.tolist()
    if isinstance(value, tuple):
        value = list(value)
    if not isinstance(value, list):
        raise ValueError(f"expected a vector, got {describe(value)}")
    flat = []
    for x in value:
        if isinstance(x, list):
            flat.extend(x)
        else:
            flat.append(x)
    return [scalar(x) for x in flat]


def sequence(value):
    if _np is not None and isinstance(value, _np.ndarray):
        value = value.tolist()
    if isinstance(value, tuple):
        value = list(value)
    if not isinstance(value, list):
        raise ValueError(f"expected a sequence of states, got {describe(value)}")
    out = []
    for x in value:
        if hasattr(x, "item") and not isinstance(x, (int, float, str, bool)):
            try:
                x = x.item()
            except Exception:
                pass
        out.append(x)
    return out


def shape(value, kind):
    if kind == "scalar":
        return scalar(value)
    if kind == "vector":
        return vector(value)
    return sequence(value)


def close(a, b, tol):
    return math.isfinite(a) and abs(a - b) <= tol * max(1.0, abs(b))


def compare(observed, expected, tol):
    if isinstance(expected, list):
        if not isinstance(observed, list) or len(observed) != len(expected):
            return False
        return all(close(o, e, tol) for o, e in zip(observed, expected))
    return close(observed, expected, tol)


def changes(states):
    return sum(1 for a, b in zip(states, states[1:]) if a != b)


def check(target, prop, form, call, tol):
    kind = call["result"]
    fill = call.get("fill", [])
    result = {"id": prop["id"], "says": prop["says"]}
    try:
        observed = shape(invoke(target, prop["args"], form, fill), kind)
        if "reduce" in prop:
            observed = sum(observed)
        if "expect" in prop:
            expected = prop["expect"]
            passed = compare(observed, expected, tol)
            result["expected"] = describe(expected)
        elif "same_as" in prop:
            other = shape(invoke(target, prop["same_as"], form, fill), kind)
            if "reduce" in prop:
                other = sum(other)
            passed = compare(observed, other, tol)
            result["expected"] = "the same result as for the swapped or transformed inputs: " + describe(other)
        elif "greater_than" in prop:
            other = shape(invoke(target, prop["greater_than"], form, fill), kind)
            passed = math.isfinite(observed) and observed > other
            result["expected"] = "greater than " + describe(other)
        elif "below" in prop:
            passed = math.isfinite(observed) and observed < prop["below"]
            result["expected"] = "below " + describe(prop["below"])
        elif "changes" in prop:
            count = changes(observed)
            passed = count == prop["changes"]
            result["expected"] = f"{prop['changes']} state changes"
            observed = f"{count} state changes: " + describe(observed)
        else:
            raise ValueError("the property states no check")
        result["observed"] = describe(observed) if not isinstance(observed, str) else clip(observed)
        result["status"] = "passed" if passed else "failed"
    except Timeout as error:
        result["status"] = "failed"
        result["observed"] = str(error)
    except Exception as error:
        result["status"] = "failed"
        result["observed"] = clip(f"raised {type(error).__name__}: {error}")
    return result


def choose_form(target, spec):
    call = spec["call"]
    first = spec["properties"][0]
    tried = []
    for form in call["forms"]:
        try:
            shape(invoke(target, first["args"], form, call.get("fill", [])), call["result"])
            return form, tried
        except Exception as error:
            tried.append({"form": form, "error": clip(f"{type(error).__name__}: {error}")})
    return None, tried


def main():
    spec = json.loads(base64.b64decode(sys.argv[1]).decode())
    signal.signal(signal.SIGALRM, _alarm)
    out = {"status": "ran", "form": None, "properties": []}
    sink = io.StringIO()
    try:
        with contextlib.redirect_stdout(sink):
            try:
                module = load(spec["workdir"], spec["file"])
                target = Target(module, spec["qualname"])
            except BaseException as error:
                out["status"] = "import_failed"
                out["error"] = clip(f"{type(error).__name__}: {error}")
                raise StopIteration
            form, tried = choose_form(target, spec)
            if form is None:
                out["status"] = "could_not_call"
                out["tried"] = tried
                raise StopIteration
            out["form"] = form
            for prop in spec["properties"]:
                out["properties"].append(check(target, prop, form, spec["call"], spec.get("tolerance", 1e-6)))
    except StopIteration:
        pass
    sys.__stdout__.write(MARK + json.dumps(out) + "\n")
    sys.__stdout__.flush()


main()
