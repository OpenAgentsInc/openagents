#!/usr/bin/env python3
"""OpenCascade-backed STEP checker with deterministic JSON output."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Dict, List, Tuple


CHECKER_VERSION = 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run OpenCascade STEP checker")
    parser.add_argument("--input", required=True, help="STEP file path")
    parser.add_argument("--output", help="Optional JSON report output path")
    return parser.parse_args()


def structural_non_manifold_edges(step_text: str) -> int:
    edge_counts: Dict[Tuple[int, int], int] = {}
    for refs in re.findall(r"POLY_LOOP\(\(([^)]*)\)\)", step_text):
        points = []
        for token in refs.split(","):
            token = token.strip()
            if not token.startswith("#"):
                points = []
                break
            try:
                points.append(int(token[1:]))
            except ValueError:
                points = []
                break
        if len(points) < 3:
            continue
        for idx, point in enumerate(points):
            nxt = points[(idx + 1) % len(points)]
            if point == nxt:
                continue
            edge = (point, nxt) if point < nxt else (nxt, point)
            edge_counts[edge] = edge_counts.get(edge, 0) + 1
    return sum(1 for count in edge_counts.values() if count != 2)


def diagnostic(code: str, severity: str, message: str, remediation_hint: str, count: int = 1) -> Dict[str, object]:
    return {
        "code": code,
        "severity": severity,
        "message": message,
        "remediation_hint": remediation_hint,
        "count": count,
    }


def base_report(source: str) -> Dict[str, object]:
    return {
        "checker_version": CHECKER_VERSION,
        "backend": "opencascade",
        "source": source,
        "passed": False,
        "solid_count": 0,
        "shell_count": 0,
        "face_count": 0,
        "poly_loop_count": 0,
        "non_manifold_edge_count": 0,
        "bbox_min_mm": None,
        "bbox_max_mm": None,
        "volume_mm3": None,
        "diagnostics": [],
    }


def run_opencascade(step_path: Path, step_text: str) -> Dict[str, object]:
    report = base_report(str(step_path))
    diagnostics: List[Dict[str, object]] = []

    report["poly_loop_count"] = len(re.findall(r"POLY_LOOP\(", step_text))
    report["face_count"] = len(re.findall(r"FACE\(", step_text))
    report["non_manifold_edge_count"] = structural_non_manifold_edges(step_text)

    try:
        from OCP.BRepCheck import BRepCheck_Analyzer  # type: ignore
        from OCP.IFSelect import IFSelect_RetDone  # type: ignore
        from OCP.STEPControl import STEPControl_Reader  # type: ignore
        from OCP.TopAbs import TopAbs_SHELL, TopAbs_SOLID  # type: ignore
        from OCP.TopExp import TopExp_Explorer  # type: ignore
    except Exception as exc:
        diagnostics.append(
            diagnostic(
                "STEP_OCCT_BACKEND_UNAVAILABLE",
                "error",
                f"OpenCascade python bindings unavailable: {exc}",
                "install OCP/pythonocc-core in CI image before running checker",
            )
        )
        report["diagnostics"] = diagnostics
        return report

    reader = STEPControl_Reader()
    read_status = reader.ReadFile(str(step_path))
    if read_status != IFSelect_RetDone:
        diagnostics.append(
            diagnostic(
                "STEP_INVALID_SOLID",
                "error",
                "OpenCascade failed to read STEP file",
                "verify STEP syntax and solid entities before retrying",
            )
        )
        report["diagnostics"] = diagnostics
        return report

    reader.TransferRoots()
    shape = reader.OneShape()

    solid_count = 0
    explorer = TopExp_Explorer(shape, TopAbs_SOLID)
    while explorer.More():
        solid_count += 1
        explorer.Next()

    shell_count = 0
    shell_explorer = TopExp_Explorer(shape, TopAbs_SHELL)
    while shell_explorer.More():
        shell_count += 1
        shell_explorer.Next()

    report["solid_count"] = solid_count
    report["shell_count"] = shell_count

    analyzer = BRepCheck_Analyzer(shape)
    if not analyzer.IsValid():
        diagnostics.append(
            diagnostic(
                "STEP_INVALID_SOLID",
                "error",
                "OpenCascade topology analyzer reported invalid shape",
                "repair invalid topology before exporting STEP again",
            )
        )

    if solid_count == 0 or shell_count == 0 or shell_count < solid_count:
        diagnostics.append(
            diagnostic(
                "STEP_MISSING_SHELL",
                "error",
                f"shell count is insufficient for solids: solids={solid_count} shells={shell_count}",
                "ensure each solid has a corresponding CLOSED_SHELL",
                count=max(1, solid_count - shell_count),
            )
        )

    if report["non_manifold_edge_count"] > 0:
        diagnostics.append(
            diagnostic(
                "STEP_NON_MANIFOLD_EDGE",
                "error",
                f"detected {report['non_manifold_edge_count']} non-manifold/open edges",
                "repair shell loops so each edge is shared by exactly two faces",
                count=int(report["non_manifold_edge_count"]),
            )
        )

    report["diagnostics"] = diagnostics
    report["passed"] = len([d for d in diagnostics if d["severity"] == "error"]) == 0
    return report


def main() -> int:
    args = parse_args()
    step_path = Path(args.input)
    if not step_path.exists():
        report = base_report(str(step_path))
        report["diagnostics"] = [
            diagnostic(
                "STEP_INPUT_NOT_FOUND",
                "error",
                f"input STEP file does not exist: {step_path}",
                "check the input path and rerun checker",
            )
        ]
        output = json.dumps(report, indent=2)
        if args.output:
            Path(args.output).write_text(output, encoding="utf-8")
        print(output)
        return 1

    step_text = step_path.read_text(encoding="utf-8", errors="replace")
    report = run_opencascade(step_path, step_text)
    output = json.dumps(report, indent=2)
    if args.output:
        Path(args.output).write_text(output, encoding="utf-8")
    print(output)
    return 0 if report.get("passed") else 2


if __name__ == "__main__":
    sys.exit(main())
