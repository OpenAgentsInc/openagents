#!/usr/bin/env python3
"""Read RSS and physical footprint for one process group on macOS.

The caller runs this read-only helper with a subprocess deadline. It never
signals a process or lists another group. The ABI follows the installed SDK's
libproc.h and sys/resource.h (RUSAGE_INFO_V0).
"""

import ctypes
import errno
import json
import os
import sys


class RusageInfoV0(ctypes.Structure):
    _fields_ = [("ri_uuid", ctypes.c_uint8 * 16)] + [
        (name, ctypes.c_uint64) for name in (
            "ri_user_time", "ri_system_time", "ri_pkg_idle_wkups", "ri_interrupt_wkups",
            "ri_pageins", "ri_wired_size", "ri_resident_size", "ri_phys_footprint",
            "ri_proc_start_abstime", "ri_proc_exit_abstime",
        )
    ]


def group_memory(group):
    if sys.platform != "darwin" or type(group) is not int or group <= 0:
        raise ValueError("a positive macOS process group is required")
    library = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
    library.proc_listpgrppids.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
    library.proc_listpgrppids.restype = ctypes.c_int
    library.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
    library.proc_pid_rusage.restype = ctypes.c_int
    capacity = 256
    while True:
        pids = (ctypes.c_int * capacity)()
        ctypes.set_errno(0)
        count = library.proc_listpgrppids(group, pids, ctypes.sizeof(pids))
        error = ctypes.get_errno()
        if count < 0 or (count == 0 and error):
            raise OSError(error, f"proc_listpgrppids failed for group {group}")
        if count < capacity:
            break
        if capacity >= 65536:
            raise RuntimeError("owned process group exceeds the bounded PID buffer")
        capacity *= 2
    members = []
    seen = set()
    for pid in pids[:count]:
        if pid <= 0 or pid in seen:
            raise RuntimeError("libproc returned invalid or duplicate process IDs")
        seen.add(pid)
        try:
            if os.getpgid(pid) != group:
                continue
        except ProcessLookupError:
            continue
        info = RusageInfoV0()
        if library.proc_pid_rusage(pid, 0, ctypes.byref(info)) != 0:
            error = ctypes.get_errno()
            if error == errno.ESRCH:
                continue
            raise OSError(error, f"proc_pid_rusage failed for owned PID {pid}")
        members.append({"pid": pid, "rss_bytes": info.ri_resident_size,
                        "physical_footprint_bytes": info.ri_phys_footprint,
                        "accounted_bytes": max(info.ri_resident_size, info.ri_phys_footprint)})
    return {"process_group": group, "members": members,
            **{key: sum(member[key] for member in members) for key in (
                "rss_bytes", "physical_footprint_bytes", "accounted_bytes")},
            "memory_accounting": "sum of max(RSS, physical footprint) for each owned group member"}


def self_test():
    from unittest.mock import patch

    assert ctypes.sizeof(RusageInfoV0) == 96 and ctypes.alignment(RusageInfoV0) == 8
    assert RusageInfoV0.ri_resident_size.offset == 64 and RusageInfoV0.ri_phys_footprint.offset == 72
    class Function:
        def __init__(self, function):
            self.function = function
        def __call__(self, *args):
            return self.function(*args)
    class Library:
        def __init__(self):
            self.calls = []
            self.proc_listpgrppids = Function(self.list_group)
            self.proc_pid_rusage = Function(self.read_usage)
        def list_group(self, group, pids, size):
            self.calls.append((group, size))
            count = min(300, size // ctypes.sizeof(ctypes.c_int))
            for index in range(count):
                pids[index] = 1000 + index
            return count
        def read_usage(self, pid, flavor, pointer):
            assert flavor == 0
            info = ctypes.cast(pointer, ctypes.POINTER(RusageInfoV0)).contents
            info.ri_resident_size = 10 if pid % 2 else 30
            info.ri_phys_footprint = 20
            return 0
    library = Library()
    with patch.object(sys, "platform", "darwin"), patch.object(ctypes, "CDLL", return_value=library), patch.object(os, "getpgid", return_value=123):
        result = group_memory(123)
        assert len(library.calls) == 2 and all(group == 123 for group, _ in library.calls)
        assert len(result["members"]) == 300
        assert result["rss_bytes"] == 6000 and result["physical_footprint_bytes"] == 6000
        assert result["accounted_bytes"] == 7500
        def denied(*args):
            ctypes.set_errno(errno.EPERM)
            return -1
        library.proc_pid_rusage = Function(denied)
        try:
            group_memory(123)
            raise AssertionError("unreadable footprint accepted")
        except OSError as error:
            assert error.errno == errno.EPERM
        library.proc_listpgrppids = Function(denied)
        try:
            group_memory(123)
            raise AssertionError("unreadable group accepted")
        except OSError as error:
            assert error.errno == errno.EPERM
    print("macOS memory ABI, scoped group growth, accounting, and refusal tests passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    elif len(sys.argv) != 2:
        raise SystemExit("usage: mac_memory.py PROCESS_GROUP")
    else:
        print(json.dumps(group_memory(int(sys.argv[1]))))
