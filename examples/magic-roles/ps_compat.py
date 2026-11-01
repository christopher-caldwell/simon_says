#!/usr/bin/env python3
"""Fail-closed macOS ps subset for the isolated native diagnostic only.

macOS refuses to execute its setuid /bin/ps inside the action sandbox. The
collector asks for process start identity and for a host Java-process snapshot.
We expose only those exact calls. We refuse to fabricate host metrics: if any
Java process is visible, host observation fails instead of reporting a number.
"""
import ctypes
import locale
import os
import subprocess
import sys
import time


class ProcBsdInfo(ctypes.Structure):
    _fields_ = [
        ("first", ctypes.c_uint32 * 12),
        ("comm", ctypes.c_char * 16),
        ("name", ctypes.c_char * 32),
        ("last", ctypes.c_uint32 * 6),
        ("start_seconds", ctypes.c_uint64),
        ("start_microseconds", ctypes.c_uint64),
    ]


def main(args):
    if sys.platform != "darwin":
        raise RuntimeError("diagnostic ps compatibility shim requires macOS")
    if len(args) == 4 and args[0] == "-p" and args[2:] == ["-o", "lstart="]:
        if not args[1].isascii() or not args[1].isdigit():
            return 1
        pid = int(args[1])
        if not 0 < pid < (1 << 31):
            return 1
        library = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
        library.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64,
                                         ctypes.c_void_p, ctypes.c_int]
        library.proc_pidinfo.restype = ctypes.c_int
        info = ProcBsdInfo()
        result = library.proc_pidinfo(pid, 3, 0, ctypes.byref(info), ctypes.sizeof(info))
        # pbi_uid is the effective UID. This fixture only needs its own processes.
        if result != ctypes.sizeof(info) or info.first[5] != os.geteuid():
            return 1
        locale.setlocale(locale.LC_TIME, "")
        print(time.strftime("%a %b %e %H:%M:%S %Y", time.localtime(info.start_seconds)))
        return 0
    if args == ["-axo", "pid,pcpu,rss,comm"]:
        result = subprocess.run(["/usr/bin/pgrep", "-x", "java"], capture_output=True, text=True)
        if result.returncode == 1:
            # The caller discards the header and selects rows whose command is
            # exactly java. With no java processes, its result is an empty list.
            print("  PID  %CPU    RSS COMM")
            return 0
        if result.returncode == 0:
            print("cannot report Java process CPU/RSS under sandbox; diagnostic stopped", file=sys.stderr)
            return 1
        print("cannot establish whether Java processes exist; diagnostic stopped", file=sys.stderr)
        return 1
    print("unsupported ps compatibility invocation", file=sys.stderr)
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Exception as error:
        print(f"ps compatibility failed: {error}", file=sys.stderr)
        sys.exit(1)
