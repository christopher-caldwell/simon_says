#!/usr/bin/env python3
"""Example domain tool: report a fixable error until its input is repaired."""
import json
from pathlib import Path
import sys

if not json.loads(Path("engine.json").read_text())["correct"]:
    print("example check failed", file=sys.stderr)
    sys.exit(7)
print("42")
