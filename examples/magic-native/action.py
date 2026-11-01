#!/usr/bin/env python3
"""Run a bounded existing Manabrew native diagnostic in an isolated action store."""
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys


def load(path):
    return json.loads(Path(path).read_text())


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def reply(packet, outcome, report, candidate=None, inspected=None):
    print(json.dumps({"action_id": packet["action_id"], "outcome": outcome,
                      "report": report, "candidate_commit": candidate, "inspected_commit": inspected}))


def invoke(cli, store, *args):
    # Use a narrow, fail-closed process-observation shim: the system ps binary
    # is setuid and cannot execute inside macOS sandbox-exec.
    shim_dir = Path(os.environ["SIMON_SCRATCH"]) / "diagnostic-bin"
    shim_dir.mkdir(exist_ok=True)
    shim = shim_dir / "ps"
    shutil.copyfile(Path(__file__).with_name("ps_compat.py"), shim)
    shim.chmod(0o755)
    env = dict(os.environ)
    env["PATH"] = str(shim_dir) + os.pathsep + env.get("PATH", "")
    process = subprocess.run([str(cli), "--local", str(store), *args], text=True,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90, env=env)
    if process.returncode:
        raise RuntimeError(f"native simulator exited {process.returncode}: {process.stderr[-2000:]}")
    return json.loads(process.stdout)


def summary(simulation):
    return {"run": simulation["run"], "eligibleGames": simulation["eligibleGames"],
            "summary": "One native diagnostic game was verified; it does not establish deck quality or certify all game rules."}


def main():
    packet_path = Path(os.environ["SIMON_PACKET"])
    packet = load(packet_path)
    runtime_root = Path(os.environ["SIMON_MAGIC_RUNTIME_ROOT"]).resolve(strict=True)
    cli = runtime_root / "repo/sim/playtest"
    gate, stage, feedback = packet["gate"], packet["stage"], packet.get("feedback", [])
    if gate == "unblocker":
        reply(packet, "retry", "The isolated diagnostic may retry once.")
        return
    if gate == "worker" and stage == "simulate":
        if not any("isolated diagnostic may retry" in item for item in feedback):
            reply(packet, "blocked", "Exercise a recoverable preflight blocker before simulation.")
            return
        revision = 2 if any("revision 2" in item for item in feedback) else 1
        store = Path(os.environ["SIMON_SCRATCH"]) / "native-store"
        store.mkdir()
        shutil.copyfile(runtime_root / "store/runtime.json", store / "runtime.json")
        result = invoke(cli, store, "run", "--config", str(Path("diagnostic.json").resolve()), "--repo", str(Path.cwd()))
        if result.get("state") != "complete" or result.get("assignedTrials") != 1 or result.get("eligibleGames") != 1:
            raise RuntimeError("native diagnostic did not complete with one eligible game")
        value = {"run": result["run"], "store": str(store), "assignedTrials": 1,
                 "eligibleGames": 1, "revision": revision, "rulesCertified": False}
        filename = "simulation.json"
    elif gate == "worker" and stage == "analyze":
        value, filename = summary(load("simulation.json")), "analysis.json"
    elif gate == "reviewer":
        head = git("rev-parse", "HEAD")
        if stage == "simulate":
            simulation = load("simulation.json")
            # The collector records absolute attempt paths and refuses silently
            # relocated stores. Copy the database into this action's writable
            # scratch, but explicitly point attempt directories at the original
            # protected evidence; the unchanged verifier checks those hashes.
            original_store = Path(simulation["store"]).resolve(strict=True)
            actions_dir = packet_path.parents[1].resolve(strict=True)
            if not original_store.is_relative_to(actions_dir):
                raise ValueError("simulation store is outside this run's actions")
            run_id = simulation["run"]
            if not isinstance(run_id, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", run_id):
                raise ValueError("invalid simulation run identity")
            verification_store = Path(os.environ["SIMON_SCRATCH"]) / "verification-store"
            shutil.copytree(original_store, verification_store)
            original_attempts = original_store / "runs" / run_id / "attempts"
            copied_attempts = verification_store / "runs" / run_id / "attempts"
            for source in original_attempts.iterdir():
                if source.is_symlink() or not source.is_dir():
                    raise ValueError("unexpected original attempt path")
                destination = copied_attempts / source.name
                if destination.is_symlink() or not destination.is_dir():
                    raise ValueError("unexpected copied attempt path")
                shutil.rmtree(destination)
                destination.symlink_to(source.resolve(strict=True), target_is_directory=True)
            verification = invoke(cli, verification_store, "verify", "--run", run_id)
            if verification.get("verified") != 1 or verification.get("eligibleGames") != 1:
                raise RuntimeError("native verification did not find one eligible verified game")
            if simulation["revision"] < 2:
                reply(packet, "changes_required", "Exercise correction: produce revision 2 and verify a new diagnostic.", inspected=head)
            else:
                reply(packet, "pass", "Native verifier confirmed one eligible game; rule completeness is not certified.", inspected=head)
        elif stage == "analyze":
            actual = load("analysis.json")
            reply(packet, "pass" if actual == summary(load("simulation.json")) else "changes_required",
                  "Checked the summary against the recorded native diagnostic.", inspected=head)
        else:
            raise ValueError("unsupported reviewer stage")
        return
    else:
        raise ValueError("unsupported action")
    Path(filename).write_text(json.dumps(value, indent=2) + "\n")
    subprocess.run(["git", "add", "--", filename], check=True)
    subprocess.run(["git", "-c", "user.name=Simon Says Native Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", f"Create {filename}"], stdout=subprocess.DEVNULL, check=True)
    reply(packet, "complete", f"Created {filename}.", candidate=git("rev-parse", "HEAD"))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"native diagnostic action failed: {error}", file=sys.stderr)
        sys.exit(1)
