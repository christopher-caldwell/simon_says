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
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
    if process.returncode:
        raise RuntimeError(f"native command failed ({process.returncode}): {process.stderr[-1500:]}")
    return json.loads(process.stdout)


def mirror_verify(collection):
    """Copy the SQLite store but verify original attempt bytes and absolute paths."""
    original = Path(collection["store"]).resolve(strict=True)
    actions = Path(os.environ["SIMON_PACKET"]).resolve().parents[1]
    if not original.is_relative_to(actions):
        raise ValueError("store must belong to the active run's actions")
    run = collection["run"]
    if not isinstance(run, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", run):
        raise ValueError("invalid run identity")
    target = Path(os.environ["SIMON_SCRATCH"]) / "verify-store"
    if target.exists():
        raise ValueError("verification scratch already exists; do not silently overwrite evidence")
    shutil.copytree(original, target)
    originals = original / "runs" / run / "attempts"
    copies = target / "runs" / run / "attempts"
    for source in originals.iterdir():
        if source.is_symlink() or not source.is_dir():
            raise ValueError("unsafe original attempt")
        dest = copies / source.name
        if dest.is_symlink() or not dest.is_dir():
            raise ValueError("unsafe copied attempt")
        shutil.rmtree(dest)  # only a freshly copied, action-owned scratch directory
        dest.symlink_to(source.resolve(strict=True), target_is_directory=True)
    result = run_native(target, "verify", "--run", run)
    if (result.get("run") != run or result.get("verified") != 1 or result.get("verifiedAttempts") != 1
            or result.get("retainedAttempts") != 1 or result.get("eligibleGames") != 1
            or result.get("rulesCertified") is not False):
        raise ValueError("native verifier did not establish exactly one retained eligible diagnostic")
    return result


def collect():
    p = packet()
    if p["stage"] != "supervise" or p["gate"] != "worker":
        raise ValueError("only the Supervisor Worker may collect")
    design = load("design.json")
    config = Path("diagnostic.json")
    if design["original_question"] != p.get("run_context") or design["config_sha256"] != hashlib.sha256(config.read_bytes()).hexdigest():
        raise ValueError("original question or frozen diagnostic configuration changed")
    if design["planned_games"] != 1 or load(config)["games"] != 1 or load(config)["purpose"] != "diagnostic":
        raise ValueError("this fixture permits one diagnostic game only")
    output = Path("collection.json")
    if output.exists():
        raise ValueError("collection already exists; do not retry or overwrite")
    runtime = Path(os.environ["SIMON_MAGIC_RUNTIME_ROOT"]).resolve(strict=True)
    store = Path(os.environ["SIMON_SCRATCH"]) / "native-store"
    store.mkdir()
    files = verify(runtime, load("runtime_source_manifest.json"))
    (store / "runtime.json").write_bytes(files["store/runtime.json"])
    result = run_native(store, "run", "--config", str(config.resolve()), "--repo", str(Path.cwd()))
    if (result.get("state") != "complete" or result.get("assignedTrials") != 1
            or result.get("attempts") != 1 or result.get("eligibleGames") != 1
            or result.get("rulesCertified") is not False):
        raise RuntimeError("native run was incomplete, excluded, retried or had unexpected certification; retain scratch and stop")
    issues = [{"kind": "exclusion", "details": result.get("exclusionReasons", {})}] if result.get("exclusionReasons") else []
    if result.get("eligibleQualityFlags"):
        issues.append({"kind": "eligible_warning", "details": result["eligibleQualityFlags"]})
    collection = {"original_question": design["original_question"], "run": result["run"], "store": str(store),
                  "assigned": result["assignedTrials"], "attempts": result["attempts"], "eligible": result["eligibleGames"],
                  "excluded": result.get("exclusionReasons", {}), "rules_certified": False, "issues": issues,
                  "runtime_identity": result["runtimeIdentity"]}
    # Collection is not a handoff until the original attempt evidence verifies.
    # The later Reviewer and Analyzer each verify independently in fresh scratch.
    mirror_verify(collection)
    output.write_text(json.dumps(collection, indent=2) + "\n")
    # Do not print results about card/deck performance to the Supervisor.
    print(json.dumps({"state": result["state"], "assigned": collection["assigned"], "attempts": collection["attempts"],
                      "eligible": collection["eligible"], "issues": len(issues), "run": collection["run"]}))


def main():
    if sys.argv[1:] == ["collect"]:
        collect()
    elif sys.argv[1:] == ["verify"]:
        result = mirror_verify(load("collection.json"))
        print(json.dumps({"verified": result.get("verified"), "eligibleGames": result.get("eligibleGames"),
                          "rulesCertified": result.get("rulesCertified", False)}))
    else:
        raise ValueError("use exactly collect or verify")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"native diagnostic helper failed: {error}", file=sys.stderr)
        sys.exit(1)
