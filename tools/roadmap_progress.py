#!/usr/bin/env python3
# tools/roadmap_progress.py
"""
Compute QuarXTor roadmap progress from ROADMAP-TARGET.md.

- Parses EPIC headers: "# EPIC E<number> (W=<int>) — ..."
- Counts Markdown checkboxes "- [ ]" and "- [x]" inside each EPIC block
- Computes:
  - Epic progress = checked/total (0 if no checkboxes)
  - Project progress = weighted average by W
- Evaluates Gate A/B/C readiness using thresholds in ROADMAP-TARGET.md:
  Gate A: E1>=95, E2>=70, E3>=60
  Gate B: E4>=90, E6>=80, E7>=70
  Gate C: E9>=80

Exit codes:
  0 = ok
  2 = parse error / missing file / missing weights

Usage:
  tools/roadmap_progress.py [--file ROADMAP-TARGET.md] [--json] [--strict]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from typing import Dict, List, Optional, Tuple


EPIC_RE = re.compile(
    r"^#\s+EPIC\s+(E\d+)\s*\(W\s*=\s*(\d+)\)\s+—\s+(.+?)\s*$"
)
CB_RE = re.compile(r"^\s*-\s*\[(?P<mark>[ xX])\]\s+(?P<text>.*)$")
# Any checkbox containing "[DROP]" anywhere in its text is excluded from totals and completion counts.
DROP_RE = re.compile(r"\[DROP\]", re.IGNORECASE)


@dataclass
class Epic:
    key: str
    weight: int
    title: str
    total: int = 0
    checked: int = 0

    @property
    def progress(self) -> float:
        if self.total <= 0:
            return 0.0
        return self.checked / self.total


@dataclass
class GateResult:
    name: str
    ok: bool
    details: Dict[str, Dict[str, float]]  # epic -> {"need":..., "have":...}


def read_text(path: str) -> str:
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


def parse_epics(md: str, strict: bool) -> List[Epic]:
    lines = md.splitlines()
    epics: List[Epic] = []
    current: Optional[Epic] = None

    for ln, line in enumerate(lines, start=1):
        m = EPIC_RE.match(line)
        if m:
            current = Epic(key=m.group(1), weight=int(m.group(2)), title=m.group(3).strip())
            epics.append(current)
            continue

        if current is None:
            continue

        cb = CB_RE.match(line)
        if cb:
            text = cb.group("text").strip()
            if DROP_RE.search(text):
                continue
            current.total += 1
            mark = cb.group("mark")
            if mark.strip().lower() == "x":
                current.checked += 1

    if strict:
        # Ensure unique epic keys and non-zero weights
        seen = set()
        for e in epics:
            if e.key in seen:
                raise ValueError(f"Duplicate EPIC key: {e.key}")
            seen.add(e.key)
            if e.weight <= 0:
                raise ValueError(f"Invalid weight for {e.key}: {e.weight}")

    return epics


def weighted_project_progress(epics: List[Epic]) -> Tuple[float, int]:
    total_w = sum(e.weight for e in epics if e.weight > 0)
    if total_w <= 0:
        return 0.0, 0
    acc = 0.0
    for e in epics:
        if e.weight > 0:
            acc += e.weight * e.progress
    return (acc / total_w) * 100.0, total_w


def epic_map(epics: List[Epic]) -> Dict[str, Epic]:
    return {e.key: e for e in epics}


def gate_check(ep: Dict[str, Epic], name: str, req: Dict[str, float]) -> GateResult:
    details: Dict[str, Dict[str, float]] = {}
    ok = True
    for k, need in req.items():
        have = ep.get(k).progress * 100.0 if k in ep else 0.0
        if k not in ep:
            ok = False
        if have + 1e-9 < need:
            ok = False
        details[k] = {"need": float(need), "have": float(have)}
    return GateResult(name=name, ok=ok, details=details)


def render_text(epics: List[Epic], proj_pct: float, gates: List[GateResult]) -> str:
    # Sort by numeric epic id
    def epic_sort_key(e: Epic) -> int:
        m = re.match(r"E(\d+)", e.key)
        return int(m.group(1)) if m else 10_000

    epics_sorted = sorted(epics, key=epic_sort_key)

    out: List[str] = []
    out.append(f"PROJECT: {proj_pct:.2f}%")
    out.append("DROP items excluded from counts.")
    out.append("")
    out.append("EPICS:")
    for e in epics_sorted:
        out.append(f"  {e.key} (W={e.weight}): {e.checked}/{e.total} = {e.progress*100.0:.2f}% — {e.title}")
    out.append("")
    out.append("GATES:")
    for g in gates:
        out.append(f"  {g.name}: {'OK' if g.ok else 'BLOCKED'}")
        for k, d in g.details.items():
            out.append(f"    {k}: have {d['have']:.2f}% / need {d['need']:.2f}%")
    return "\n".join(out) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", default="ROADMAP-TARGET.md", help="Path to ROADMAP-TARGET.md")
    ap.add_argument("--json", action="store_true", help="Emit JSON instead of text")
    ap.add_argument("--strict", action="store_true", help="Fail on duplicate epics / invalid weights")
    args = ap.parse_args()

    path = args.file
    if not os.path.exists(path):
        print(f"ERROR: file not found: {path}", file=sys.stderr)
        return 2

    try:
        md = read_text(path)
        epics = parse_epics(md, strict=args.strict)
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2

    ep = epic_map(epics)
    proj_pct, total_w = weighted_project_progress(epics)

    # Gates per your ROADMAP-TARGET.md spec
    gates = [
        gate_check(ep, "Gate A", {"E1": 95.0, "E2": 70.0, "E3": 60.0}),
        gate_check(ep, "Gate B", {"E4": 90.0, "E6": 80.0, "E7": 70.0}),
        gate_check(ep, "Gate C", {"E9": 80.0}),
    ]

    if args.json:
        payload = {
            "project_percent": proj_pct,
            "total_weight": total_w,
            "epics": [
                {
                    "key": e.key,
                    "weight": e.weight,
                    "title": e.title,
                    "checked": e.checked,
                    "total": e.total,
                    "percent": e.progress * 100.0,
                }
                for e in epics
            ],
            "gates": [
                {"name": g.name, "ok": g.ok, "details": g.details}
                for g in gates
            ],
        }
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print(render_text(epics, proj_pct, gates), end="")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
