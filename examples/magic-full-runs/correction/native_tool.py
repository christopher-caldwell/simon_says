#!/usr/bin/env python3
"""Bounded native diagnostic helper. Never retries, repairs, publishes or retires."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
from runtime_source import regular_bytes, verify


def load(path):
    return json.loads(Path(path).read_text())


def packet():
    return load(os.environ["SIMON_PACKET"])


def run_native(store, *args):
    runtime = Path(os.environ["SIMON_MAGIC_RUNTIME_ROOT"]).resolve(strict=True)
    files = verify(runtime, load("runtime_source_manifest.json"))
    if regular_bytes(Path(store) / "runtime.json") != files["store/runtime.json"]:
        raise ValueError("action store runtime receipt differs from the frozen runtime authority")
    scratch = Path(os.environ["SIMON_SCRATCH"])
    source_copy = Path(tempfile.mkdtemp(prefix="verified-native-source-", dir=scratch))
    for name, data in files.items():
        if name.startswith("repo/sim/"):
            target = source_copy / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
    cli = source_copy / "repo/sim/playtest"
    cli.chmod(0o755)
    shim_dir = Path(os.environ["SIMON_SCRATCH"]) / "bin"
    shim_dir.mkdir(exist_ok=True)
    shim = shim_dir / "ps"
    shutil.copyfile(Path(__file__).with_name("ps_compat.py"), shim)
    shim.chmod(0o755)
    env = dict(os.environ, PATH=str(shim_dir) + os.pathsep + os.environ.get("PATH", ""))
    env.pop("PYTHONPATH", None)
    env.pop("PYTHONHOME", None)
    env["PYTHONNOUSERSITE"] = "1"
    env["PYTHONPYCACHEPREFIX"] = tempfile.mkdtemp(prefix="native-pycache-", dir=scratch)
    process = subprocess.run([str(cli), "--local", str(store), *args], env=env, text=True,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=550)
    if process.returncode:
        raise RuntimeError(f"native command failed ({process.returncode}): {process.stderr[-1500:]}")
    return json.loads(process.stdout)


def evidence():
    manifest = load("evidence-manifest.json")
    root = Path(manifest["store"]).resolve(strict=True)
    actual = {}
    for f in sorted(root.rglob("*")):
        if f.is_symlink():
            raise ValueError("evidence symlink")
        if f.is_file():
            actual[str(f.relative_to(root))] = hashlib.sha256(f.read_bytes()).hexdigest()
    if actual != manifest["files"]:
        raise ValueError("original evidence differs from frozen manifest")
    return root, manifest["run"]


def mirror_verify(collection):
    original, run = evidence()
    if str(original) != collection["store"] or run != collection["run"]:
        raise ValueError("collection evidence identity mismatch")
    scratch = Path(tempfile.mkdtemp(prefix="evidence-verify-", dir=os.environ["SIMON_SCRATCH"]))
    target = scratch / "store"
    shutil.copytree(original, target)
    originals = original / "runs" / run / "attempts"
    copies = target / "runs" / run / "attempts"
    for source in originals.iterdir():
        if source.is_symlink() or not source.is_dir():
            raise ValueError("unsafe original attempt")
        dest = copies / source.name
        if dest.is_symlink() or not dest.is_dir():
            raise ValueError("unsafe copied attempt")
        shutil.rmtree(dest)
        dest.symlink_to(source.resolve(strict=True), target_is_directory=True)
    result = run_native(target, "verify", "--run", run)
    if (result.get("run") != run or result.get("verified") != collection["attempts"] or
        result.get("verifiedAttempts") != collection["attempts"] or result.get("retainedAttempts") != collection["attempts"] or
        result.get("eligibleGames") != collection["eligible"] or result.get("rulesCertified") is not False):
        raise ValueError("native verifier returned unexpected diagnostic denominators")
    evidence()
    return result


if __name__ == "__main__":
    if sys.argv[1:] != ["verify"]:
        sys.exit("This analysis-only tool supports only verify, never collection")
    try:
        result = mirror_verify(load("collection.json"))
        print(json.dumps({key: result[key] for key in ("run", "verified", "verifiedAttempts", "retainedAttempts", "eligibleGames", "rulesCertified")}))
    except Exception as error:
        print(f"evidence verification failed: {error}", file=sys.stderr)
        sys.exit(1)
