#!/usr/bin/env python3
"""Create a disposable copy of the recorded engine for the requested repair."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tomllib

BASELINE = "44f09e473213606382168f0b08442057823b9128"


def git(*args, cwd=None):
    return subprocess.run(["git", *map(str, args)], cwd=cwd, text=True, check=True,
                          stdout=subprocess.PIPE).stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("engine_repo", type=Path, help="local Manabrew checkout, read-only")
    parser.add_argument("collection_repo", type=Path, help="Magic collection checkout, read-only")
    parser.add_argument("new_project", type=Path, help="new isolated clone, outside both sources")
    args = parser.parse_args()
    engine = Path(git("-C", args.engine_repo.resolve(strict=True), "rev-parse", "--show-toplevel"))
    collection = args.collection_repo.resolve(strict=True)
    requested = args.new_project.absolute()
    target = requested.parent.resolve(strict=True) / requested.name
    if target.exists() or target.is_symlink() or target.is_relative_to(engine) or target.is_relative_to(collection):
        raise ValueError("destination must be new and outside both source checkouts")
    commit = git("-C", engine, "rev-parse", "--verify", BASELINE + "^{commit}")
    if commit != BASELINE:
        raise ValueError("recorded historical engine baseline missing")
    role_path = collection / ".codex/agents/fixer.toml"
    raw = role_path.read_bytes()
    role = tomllib.loads(raw.decode())
    git("clone", "--no-hardlinks", "--no-checkout", "--", engine, target)
    git("checkout", "--detach", commit, cwd=target)
    git("config", "user.name", "Simon Says Isolated Repair", cwd=target)
    git("config", "user.email", "simon-repair@example.invalid", cwd=target)
    # The checkpoint engine rejects Git submodules. This recorded gitlink is
    # unused by the Rust repair; remove only its entry in the disposable clone.
    if git("ls-files", "--stage", "--", "forge", cwd=target).startswith("160000 "):
        git("rm", "--cached", "--", "forge", cwd=target)
    templates = Path(__file__).resolve().parent
    dest = target / "simon-repair"
    dest.mkdir()
    for name in ("fix.md", "issue.md", "schema.json", "review.py", "stop.py"):
        shutil.copyfile(templates / name, dest / name)
    (dest / "profile.md").write_text(
        "# Scoped Fixer role\n\nYour only authority is the explicit isolated repair request in fix.md. "
        "Historical role references to live source, installation, no commits, dispatch or publication "
        "are superseded solely for this disposable checkpoint. Never access other checkouts or alter evidence.\n\n"
        f"Source profile SHA256 {hashlib.sha256(raw).hexdigest()}\n\n```toml\n{raw.decode()}\n```\n")
    workflow = (templates / "workflow.toml").read_text()
    workflow = workflow.replace('model = "gpt-6.1-terra"', 'model = ' + json.dumps(role["model"]))
    workflow = workflow.replace('effort = "high"', 'effort = ' + json.dumps(role["model_reasoning_effort"]))
    (target / "simon-workflow.toml").write_text(workflow)
    if role_path.read_bytes() != raw:
        raise RuntimeError("source role changed while preparing; inspect the incomplete isolated clone")
    git("add", "simon-repair", "simon-workflow.toml", cwd=target)
    git("commit", "-m", "Prepare isolated historical engine repair", cwd=target)
    print(json.dumps({"project": str(target), "source_commit": commit,
                      "baseline": git("rev-parse", "HEAD", cwd=target),
                      "profile_sha256": hashlib.sha256(raw).hexdigest(),
                      "model": role["model"], "effort": role["model_reasoning_effort"]}))


if __name__ == "__main__":
    main()
