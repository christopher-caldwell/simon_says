#!/usr/bin/env python3
"""Create a disposable repair repository using the user's current Fixer profile."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("collection_repo", type=Path, help="read-only Magic collection checkout")
parser.add_argument("new_project", type=Path, help="new isolated project outside the checkout")
args = parser.parse_args()
source = args.collection_repo.resolve(strict=True)
requested = args.new_project.absolute()
target = requested.parent.resolve(strict=True) / requested.name
if target.exists() or target.is_symlink() or target.is_relative_to(source):
    raise ValueError("target must be new and outside the source checkout")
role_path = source / ".codex/agents/fixer.toml"
raw = role_path.read_bytes()
role = tomllib.loads(raw.decode())
head = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
target.mkdir()
for name in ("collector.py", "test_collector.py", "issue.md", "fix.md", "repair.schema.json", "review.py", "unblock.py", "workflow.toml"):
    shutil.copy2(Path(__file__).resolve().parent / name, target / name)
profile = ("# Adapted Fixer profile for isolated explicit repair\n\n"
    "Follow the separately requested fixture scope in issue.md and fix.md. "
    "The original profile below is frozen historical context. References to the live production backlog, "
    "native engine acceptance, or a no-commit rule are superseded solely by this explicit disposable fixture request; "
    "the controller requires local candidate commits. Do not access the source checkout.\n\n"
    f"Source HEAD: {head} (the source file may include uncommitted changes)\n"
    f"Source .codex/agents/fixer.toml SHA-256: {hashlib.sha256(raw).hexdigest()}\n\n"
    "```toml\n" + raw.decode() + "\n```\n")
(target / "profile.md").write_text(profile)
workflow = (target / "workflow.toml").read_text()
workflow = workflow.replace('model = "gpt-6.1-terra"', 'model = ' + json.dumps(role["model"]))
workflow = workflow.replace('effort = "high"', 'effort = ' + json.dumps(role["model_reasoning_effort"]))
(target / "workflow.toml").write_text(workflow)
if role_path.read_bytes() != raw or subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip() != head:
    raise RuntimeError("source changed during preparation; discard this incomplete fixture and retry")
for command in (["init", "-q"], ["config", "user.name", "Simon Says Fixture"], ["config", "user.email", "simon-fixture@example.invalid"], ["add", "."], ["commit", "-qm", "Prepare isolated requested repair"]):
    subprocess.run(["git", "-C", str(target), *command], check=True)
print(json.dumps({"project": str(target), "source_head": head, "profile_sha256": hashlib.sha256(raw).hexdigest(), "model": role["model"], "effort": role["model_reasoning_effort"]}))
