"""Capture and verify the native Python launcher/source and runtime receipt."""
import hashlib
import os
from pathlib import Path
import stat


def regular_bytes(path):
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError(f"runtime authority is not a regular non-symlink file: {path}")
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(fd, "rb") as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError(f"runtime authority is not regular: {path}")
        return stream.read()


def capture(root):
    root = Path(root).resolve(strict=True)
    sim = root / "repo/sim"
    package = sim / "manabrew"
    for path in (root / "repo", sim, package, root / "store"):
        if path.is_symlink() or not path.is_dir():
            raise ValueError(f"unsafe runtime directory: {path}")
    files = {"repo/sim/playtest": regular_bytes(sim / "playtest"),
             "store/runtime.json": regular_bytes(root / "store/runtime.json")}
    for current, dirs, names in os.walk(package, followlinks=False):
        base = Path(current)
        retained = []
        for name in dirs:
            if name == "__pycache__" or name == ".git":
                continue
            path = base / name
            if path.is_symlink():
                raise ValueError(f"symlinked runtime source directory: {path}")
            retained.append(name)
        dirs[:] = retained
        for name in names:
            if name.endswith(".py"):
                path = base / name
                files[path.relative_to(root).as_posix()] = regular_bytes(path)
    if "repo/sim/manabrew/__init__.py" not in files:
        raise ValueError("native Python package is incomplete")
    return files


def digest_manifest(files):
    return {"schema": 1, "files": {path: hashlib.sha256(data).hexdigest() for path, data in sorted(files.items())}}


def verify(root, manifest):
    if manifest.get("schema") != 1 or not isinstance(manifest.get("files"), dict):
        raise ValueError("invalid runtime source manifest")
    files = capture(root)
    if digest_manifest(files) != manifest:
        raise ValueError("native launcher, source set or runtime receipt changed since preparation")
    return files
