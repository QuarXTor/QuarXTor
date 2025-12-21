#!/usr/bin/env python3
# tools/roadmap_next.py
"""
Pick the next work item from ROADMAP-TARGET.md.

Selection rule (deterministic):
1) Determine earliest blocked gate: A, then B, then C. If all OK -> continue with lowest epic incomplete.
2) Within the selected "scope" epics (gate-required epics or all epics), pick the lowest-number epic with progress < 100%.
3) Inside that epic, pick the first unchecked checkbox line "- [ ] ...", returning its text + the nearest preceding section header.

Outputs:
- default text: EPIC + header context + checkbox line
- --json for tooling
- --print-range prints the file line range around the task (context window)

Usage:
  tools/roadmap_next.py [--file ROADMAP-TARGET.md] [--json] [--print-range]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from typing import Dict, List, Optional, Tuple

EPIC_RE = re.compile(r"^#\s+EPIC\s+(E\d+)\s*\(W\s*=\s*(\d+)\)\s+—\s+(.+?)\s*$")
CB_RE = re.compile(r"^\s*-\s*\[(?P<mark>[ xX])\]\s+(?P<text>.*)$")
HDR_RE = re.compile(r"^(#{2,6})\s+(.+?)\s*$")  # section headers inside epic


@dataclass
class EpicSpan:
    key: str
    weight: int
    title: str
    start_line: int
    end_line: int  # inclusive


@dataclass
class NextItem:
    epic: str
    epic_title: str
    section_header: Optional[str]
    checkbox_text: str
    line_no: int
    context_start: int
    context_end: int


def read_lines(path: str) -> List[str]:
    with open(path, "r", encoding="utf-8") as f:
        return f.read().splitlines()


def parse_epic_spans(lines: List[str]) -> List[EpicSpan]:
    spans: List[EpicSpan] = []
    current: Optional[EpicSpan] = None

    for idx, line in enumerate(lines, start=1):
        m = EPIC_RE.match(line)
        if m:
            if current is not None:
                current.end_line = idx - 1
            current = EpicSpan(
                key=m.group(1),
                weight=int(m.group(2)),
                title=m.group(3).strip(),
                start_line=idx,
                end_line=len(lines),
            )
            spans.append(current)

    if spans:
        spans[-1].end_line = len(lines)

    return spans


def count_checkboxes(lines: List[str], start: int, end: int) -> Tuple[int, int]:
    total = 0
    checked = 0
    for i in range(start, end + 1):
        cb = CB_RE.match(lines[i - 1])
        if cb:
            total += 1
            mark = cb.group("mark")
            if mark.strip().lower() == "x":
                checked += 1
    return checked, total


def epic_progress_map(lines: List[str], spans: List[EpicSpan]) -> Dict[str, Tuple[int, int, float]]:
    mp: Dict[str, Tuple[int, int, float]] = {}
    for s in spans:
        checked, total = count_checkboxes(lines, s.start_line, s.end_line)
        pct = (checked / total) * 100.0 if total else 0.0
        mp[s.key] = (checked, total, pct)
    return mp


def gate_status(ep: Dict[str, Tuple[int, int, float]]) -> Dict[str, bool]:
    def have(k: str) -> float:
        return ep.get(k, (0, 0, 0.0))[2]

    gate_a = have("E1") >= 95.0 and have("E2") >= 70.0 and have("E3") >= 60.0
    gate_b = have("E4") >= 90.0 and have("E6") >= 80.0 and have("E7") >= 70.0
    gate_c = have("E9") >= 80.0
    return {"A": gate_a, "B": gate_b, "C": gate_c}


def pick_scope(gates: Dict[str, bool]) -> Tuple[str, List[str]]:
    # earliest blocked
    if not gates["A"]:
        return "Gate A", ["E1", "E2", "E3"]
    if not gates["B"]:
        return "Gate B", ["E4", "E6", "E7"]
    if not gates["C"]:
        return "Gate C", ["E9"]
    return "All gates OK", []  # means: all epics eligible


def epic_sort_key(epic_key: str) -> int:
    m = re.match(r"E(\d+)", epic_key)
    return int(m.group(1)) if m else 10_000


def find_first_unchecked_in_epic(lines: List[str], span: EpicSpan) -> Optional[Tuple[int, Optional[str], str]]:
    # Track nearest header within epic
    current_header: Optional[str] = None
    for i in range(span.start_line, span.end_line + 1):
        line = lines[i - 1]
        hm = HDR_RE.match(line)
        if hm:
            # keep full header text without hashes
            current_header = hm.group(2).strip()
            continue
        cb = CB_RE.match(line)
        if cb and cb.group("mark").strip() == "":
            return (i, current_header, cb.group("text").strip())
    return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", default="ROADMAP-TARGET.md")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--print-range", action="store_true", help="Print context window around next item")
    ap.add_argument("--context", type=int, default=8, help="Lines of context before/after for --print-range")
    args = ap.parse_args()

    if not os.path.exists(args.file):
        print(f"ERROR: file not found: {args.file}", file=sys.stderr)
        return 2

    lines = read_lines(args.file)
    spans = parse_epic_spans(lines)
    if not spans:
        print("ERROR: No EPIC headers found.", file=sys.stderr)
        return 2

    prog = epic_progress_map(lines, spans)
    gates = gate_status(prog)
    scope_name, scope_epics = pick_scope(gates)

    # Determine candidate epics
    candidates: List[EpicSpan] = []
    if scope_epics:
        for s in spans:
            if s.key in scope_epics:
                candidates.append(s)
    else:
        candidates = spans[:]

    # Pick first epic with incomplete progress
    candidates_sorted = sorted(candidates, key=lambda s: epic_sort_key(s.key))
    chosen: Optional[EpicSpan] = None
    for s in candidates_sorted:
        _, total, pct = prog.get(s.key, (0, 0, 0.0))
        # if epic has 0 checkboxes, treat as incomplete (requires author attention)
        if total == 0 or pct < 100.0 - 1e-9:
            chosen = s
            break

    if chosen is None:
        # all candidates complete; if scope limited, widen to all
        if scope_epics:
            candidates_sorted = sorted(spans, key=lambda s: epic_sort_key(s.key))
            for s in candidates_sorted:
                _, total, pct = prog.get(s.key, (0, 0, 0.0))
                if total == 0 or pct < 100.0 - 1e-9:
                    chosen = s
                    scope_name = "All gates OK"
                    break

    if chosen is None:
        print("No remaining work items found (all epics 100%).")
        return 0

    hit = find_first_unchecked_in_epic(lines, chosen)
    if hit is None:
        # No unchecked boxes but epic incomplete (likely 0 checkboxes): report
        if args.json:
            print(json.dumps({
                "scope": scope_name,
                "epic": chosen.key,
                "epic_title": chosen.title,
                "note": "Epic has no unchecked checkbox lines (maybe 0 checkboxes). Add atomic items or mark DROP explicitly."
            }, indent=2))
        else:
            print(f"SCOPE: {scope_name}")
            print(f"EPIC: {chosen.key} — {chosen.title}")
            print("NOTE: Epic has no unchecked checkboxes (maybe 0 checkboxes). Add atomic items or mark DROP explicitly.")
        return 0

    line_no, hdr, cb_text = hit
    ctx = args.context
    ctx_start = max(1, line_no - ctx)
    ctx_end = min(len(lines), line_no + ctx)

    item = NextItem(
        epic=chosen.key,
        epic_title=chosen.title,
        section_header=hdr,
        checkbox_text=cb_text,
        line_no=line_no,
        context_start=ctx_start,
        context_end=ctx_end,
    )

    if args.json:
        print(json.dumps({
            "scope": scope_name,
            "epic": item.epic,
            "epic_title": item.epic_title,
            "section_header": item.section_header,
            "checkbox_text": item.checkbox_text,
            "line_no": item.line_no,
            "context_start": item.context_start,
            "context_end": item.context_end,
            "gates": gates,
            "epic_progress": {
                k: {"checked": v[0], "total": v[1], "percent": v[2]}
                for k, v in prog.items()
            },
        }, indent=2, sort_keys=True))
    else:
        print(f"SCOPE: {scope_name}")
        print(f"EPIC:  {item.epic} — {item.epic_title}")
        if item.section_header:
            print(f"SECT:  {item.section_header}")
        print(f"NEXT:  - [ ] {item.checkbox_text}")
        print(f"LINE:  {item.line_no}")
        if args.print_range:
            print("")
            for i in range(item.context_start, item.context_end + 1):
                prefix = ">>" if i == item.line_no else "  "
                print(f"{prefix} {i:5d}: {lines[i - 1]}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
