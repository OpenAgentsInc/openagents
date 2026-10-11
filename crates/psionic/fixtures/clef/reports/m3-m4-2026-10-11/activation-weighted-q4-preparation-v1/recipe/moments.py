"""CPU-only transactional activation moments for a frozen calibration run."""
import hashlib
import numpy as np


def require(value, message):
    if not value:
        raise ValueError(message)


class Moments:
    def __init__(self, widths):
        require(widths and all(isinstance(k, str) and type(v) is int and 0 < v <= 17408 for k, v in widths.items()), "invalid moment widths")
        self.widths = dict(widths)
        self.sums = {name: np.zeros(width, dtype=np.float64) for name, width in widths.items()}
        self.counts = {name: 0 for name in widths}
        self.completed = []
        self.pending = None

    def begin(self, identity, rows):
        require(self.pending is None and type(rows) is int and 0 < rows <= 1024, "invalid request lifecycle or token count")
        require(isinstance(identity, str) and len(identity) == 64 and identity not in self.completed, "duplicate or invalid request identity")
        self.pending = {"identity": identity, "rows": rows, "values": {}, "input_sha256": {}}

    def observe_bf16(self, name, immutable, rows, width):
        require(self.pending is not None and name in self.widths and name not in self.pending["values"], "unknown or repeated activation hook")
        require(type(immutable) is bytes and width == self.widths[name] and rows == self.pending["rows"], "activation shape differs")
        require(len(immutable) == rows * width * 2, "activation byte count differs")
        original = np.frombuffer(immutable, dtype="<u2").reshape(rows, width)
        total = np.zeros(width, dtype=np.float64)
        for first in range(0, rows, 64):
            values = (original[first:first+64].astype(np.uint32) << 16).view(np.float32)
            require(np.isfinite(values).all(), "nonfinite activation")
            # The rows are summed in float64; no BF16 arithmetic or graph survives.
            total += np.square(values, dtype=np.float64).sum(axis=0, dtype=np.float64)
        require(np.isfinite(total).all(), "moment overflow")
        self.pending["values"][name] = total
        self.pending["input_sha256"][name] = hashlib.sha256(immutable).hexdigest()

    def abort(self):
        self.pending = None

    def commit(self):
        require(self.pending is not None and set(self.pending["values"]) == set(self.widths), "incomplete activation hooks")
        for name in self.widths:
            if ".ffn_gate." in name:
                other = name.replace(".ffn_gate.", ".ffn_up.")
                require(other in self.widths and self.pending["input_sha256"][name] == self.pending["input_sha256"][other], "gate and up inputs differ")
        # Validate every prospective sum before publishing any entry.
        merged = {name: self.sums[name] + value for name, value in self.pending["values"].items()}
        require(all(np.isfinite(value).all() for value in merged.values()), "accumulated moment overflow")
        rows = self.pending["rows"]
        identity = self.pending["identity"]
        self.sums = merged
        self.counts = {name: value + rows for name, value in self.counts.items()}
        self.completed.append(identity)
        self.pending = None
        return identity

    def exported(self):
        require(self.pending is None and self.completed, "statistics incomplete")
        result = {}
        for name, total in self.sums.items():
            require(self.counts[name] > 0, "statistics have no rows")
            mean = (total / self.counts[name]).astype(np.float32)
            require(np.isfinite(mean).all() and np.all(mean >= 0) and np.any(mean > 0), "importance vector is invalid or empty")
            result[name] = mean
        return result
