#!/usr/bin/env python3
# Firm reason: 6 (ifcopenshell is a Python/C++ SDK with no C API we can call through #foreign)
"""Convert an IFC model (or deterministic sample) into a ple-scene.v1 JSON."""

import argparse
import json
import os
import sys
from typing import Any, Dict, List, Optional


LAYERS = (
    "architecture",
    "racks",
    "cooling",
    "power",
    "fire",
    "comms",
    "clash",
)


def _round3(v: float) -> float:
    return round(float(v), 3)


def _make_element(
    elem_id: str,
    layer: str,
    ifc_class: str,
    shape: str,
    center: List[float],
    size: List[float],
    rot_z: float = 0.0,
    meta: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    if layer not in LAYERS:
        raise ValueError(f"Invalid layer: {layer}")
    if shape not in ("box", "cylinder"):
        raise ValueError(f"Invalid shape: {shape}")
    return {
        "id": elem_id,
        "layer": layer,
        "ifc_class": ifc_class,
        "shape": shape,
        "center": [_round3(c) for c in center],
        "size": [_round3(s) for s in size],
        "rot_z": _round3(rot_z),
        "meta": meta or {},
    }


def build_sample_scene(task: str = "data-hall-mep-routing") -> Dict[str, Any]:
    elements: List[Dict[str, Any]] = []

    # Raised floor (27.0 x 21.0 x 0.6 m, top at z=0.6)
    elements.append(
        _make_element(
            "floor_raised_01",
            "architecture",
            "IfcSlab",
            "box",
            [13.5, 10.5, 0.3],
            [27.0, 21.0, 0.6],
            0.0,
            {"seed": "hall_seed000_d000", "role": "raised_floor"},
        )
    )

    # 8 structural columns (2 rows of 4)
    col_xs = [3.0, 10.0, 17.0, 24.0]
    col_ys = [2.5, 18.5]
    col_idx = 1
    for y in col_ys:
        for x in col_xs:
            elements.append(
                _make_element(
                    f"col_{col_idx:02d}",
                    "architecture",
                    "IfcColumn",
                    "box",
                    [x, y, 2.25],
                    [0.5, 0.5, 4.5],
                    0.0,
                    {"grid": f"C{col_idx}"},
                )
            )
            col_idx += 1

    # 128 server racks in 4 rows of 32 (0.6 x 1.2 x 2.2 m)
    row_ys = [5.5, 8.5, 12.5, 15.5]
    for r_idx, ry in enumerate(row_ys, start=1):
        for c_idx in range(32):
            rx = 3.9 + c_idx * 0.62
            elements.append(
                _make_element(
                    f"rack_r{r_idx}_c{c_idx + 1:02d}",
                    "racks",
                    "IfcFurniture",
                    "box",
                    [rx, ry, 1.7],
                    [0.6, 1.2, 2.2],
                    0.0,
                    {"row": r_idx, "pos": c_idx + 1},
                )
            )

    # 4 CRAHs along the perimeter
    crah_coords = [(1.5, 5.5), (1.5, 15.5), (25.5, 5.5), (25.5, 15.5)]
    for idx, (cx, cy) in enumerate(crah_coords, start=1):
        elements.append(
            _make_element(
                f"crah_{idx:02d}",
                "cooling",
                "IfcUnitaryEquipment",
                "box",
                [cx, cy, 1.7],
                [1.2, 2.4, 2.2],
                0.0,
                {"capacity_kw": 120},
            )
        )

    # 4 PDUs
    pdu_coords = [(1.5, 8.5), (1.5, 12.5), (25.5, 8.5), (25.5, 12.5)]
    for idx, (px, py) in enumerate(pdu_coords, start=1):
        elements.append(
            _make_element(
                f"pdu_{idx:02d}",
                "power",
                "IfcElectricDistributionBoard",
                "box",
                [px, py, 1.6],
                [1.0, 1.0, 2.0],
                0.0,
                {"kva": 300},
            )
        )

    # Sprinkler grid (fire)
    sprinkler_idx = 1
    for sx in [4.5, 10.5, 16.5, 22.5]:
        for sy in [5.5, 10.5, 15.5]:
            elements.append(
                _make_element(
                    f"sprinkler_{sprinkler_idx:02d}",
                    "fire",
                    "IfcFireSuppressionTerminal",
                    "cylinder",
                    [sx, sy, 4.2],
                    [0.15, 0.15, 0.25],
                    0.0,
                    {"k_factor": 5.6},
                )
            )
            sprinkler_idx += 1

    # Overhead cable tray (comms)
    for r_idx, ry in enumerate(row_ys, start=1):
        elements.append(
            _make_element(
                f"cable_tray_{r_idx:02d}",
                "comms",
                "IfcCableCarrierSegment",
                "box",
                [13.5, ry, 3.35],
                [20.0, 0.45, 0.1],
                0.0,
                {"row": r_idx},
            )
        )

    # Power busway
    for r_idx, ry in enumerate(row_ys, start=1):
        elements.append(
            _make_element(
                f"busway_{r_idx:02d}",
                "power",
                "IfcCableSegment",
                "box",
                [13.5, ry, 3.65],
                [20.0, 0.2, 0.18],
                0.0,
                {"row": r_idx, "amps": 800},
            )
        )

    # Chilled-water pipe cylinders (cooling)
    for idx, py in enumerate([4.0, 17.0], start=1):
        elements.append(
            _make_element(
                f"chw_pipe_{idx:02d}",
                "cooling",
                "IfcPipeSegment",
                "cylinder",
                [13.5, py, 3.9],
                [0.25, 0.25, 22.0],
                1.571,
                {"medium": "chilled_water"},
            )
        )

    # 2 sample clash markers
    elements.append(
        _make_element(
            "clash_01",
            "clash",
            "IfcVirtualElement",
            "box",
            [10.0, 5.5, 3.5],
            [0.4, 0.4, 0.4],
            0.0,
            {"severity": "hard", "pair": ["cable_tray_01", "busway_01"]},
        )
    )
    elements.append(
        _make_element(
            "clash_02",
            "clash",
            "IfcVirtualElement",
            "box",
            [16.5, 15.5, 3.9],
            [0.35, 0.35, 0.35],
            0.0,
            {"severity": "clearance", "pair": ["sprinkler_09", "chw_pipe_02"]},
        )
    )

    return {
        "version": "ple-scene.v1",
        "task": task,
        "units": "meters",
        "bounds": {"min": [0.0, 0.0, 0.0], "max": [27.0, 21.0, 4.5]},
        "run": {
            "score": 0.84,
            "gates": {"schema": True, "clash": False, "connectivity": True},
            "total_length_m": 142.5,
        },
        "elements": elements,
    }


def _classify_ifc(ifc_class: str) -> str:
    cls = ifc_class.lower()
    if any(k in cls for k in ("slab", "wall", "column", "beam", "roof", "door", "window")):
        return "architecture"
    if "furniture" in cls or "rack" in cls:
        return "racks"
    if any(k in cls for k in ("pipe", "duct", "unitary", "chiller", "fan", "airterminal")):
        return "cooling"
    if any(k in cls for k in ("electric", "cable", "transformer", "switch")):
        return "power"
    if "fire" in cls or "sprinkler" in cls:
        return "fire"
    return "comms"


def build_scene_from_ifc(
    ifc_path: str,
    task: str,
    verifier_path: Optional[str] = None,
) -> Dict[str, Any]:
    try:
        import ifcopenshell  # type: ignore
        import ifcopenshell.geom  # type: ignore
    except ImportError as exc:
        raise RuntimeError(
            "ifcopenshell is required to parse IFC files; use --sample to generate without it."
        ) from exc

    model = ifcopenshell.open(ifc_path)
    settings = ifcopenshell.geom.settings()
    settings.set(settings.USE_WORLD_COORDS, True)

    elements: List[Dict[str, Any]] = []
    min_pt = [float("inf"), float("inf"), float("inf")]
    max_pt = [float("-inf"), float("-inf"), float("-inf")]

    for product in model.by_type("IfcProduct"):
        if product.Representation is None:
            continue
        try:
            shape = ifcopenshell.geom.create_shape(settings, product)
        except Exception:
            continue
        verts = shape.geometry.verts
        if not verts:
            continue
        xs = verts[0::3]
        ys = verts[1::3]
        zs = verts[2::3]
        lo = [min(xs), min(ys), min(zs)]
        hi = [max(xs), max(ys), max(zs)]
        for i in range(3):
            min_pt[i] = min(min_pt[i], lo[i])
            max_pt[i] = max(max_pt[i], hi[i])
        center = [(lo[i] + hi[i]) * 0.5 for i in range(3)]
        size = [max(hi[i] - lo[i], 0.01) for i in range(3)]
        ifc_class = product.is_a()
        elements.append(
            _make_element(
                str(product.GlobalId),
                _classify_ifc(ifc_class),
                ifc_class,
                "box",
                center,
                size,
                0.0,
                {"name": getattr(product, "Name", None) or ""},
            )
        )

    if not elements:
        min_pt = [0.0, 0.0, 0.0]
        max_pt = [0.0, 0.0, 0.0]

    run_info: Dict[str, Any] = {
        "score": 1.0,
        "gates": {"schema": True, "clash": True, "connectivity": True},
        "total_length_m": 0.0,
    }
    if verifier_path:
        with open(verifier_path, "r", encoding="utf-8") as f:
            verifier_data = json.load(f)
        if isinstance(verifier_data, dict):
            run_info.update(verifier_data)

    return {
        "version": "ple-scene.v1",
        "task": task,
        "units": "meters",
        "bounds": {"min": [_round3(v) for v in min_pt], "max": [_round3(v) for v in max_pt]},
        "run": run_info,
        "elements": elements,
    }


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Convert IFC to ple-scene.v1 JSON.")
    parser.add_argument("input", nargs="?", help="Input IFC file path")
    parser.add_argument("-o", "--output", required=True, help="Output .ple-scene.json path")
    parser.add_argument("--verifier", help="Optional verifier JSON path")
    parser.add_argument("--task", default="data-hall-mep-routing", help="Task name")
    parser.add_argument("--sample", action="store_true", help="Generate deterministic sample scene")
    args = parser.parse_args(argv)

    if args.sample:
        scene = build_sample_scene(task=args.task)
    else:
        if not args.input:
            parser.error("INPUT.ifc is required unless --sample is specified")
        scene = build_scene_from_ifc(args.input, task=args.task, verifier_path=args.verifier)

    out_dir = os.path.dirname(os.path.abspath(args.output))
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)
    with open(args.output, "w", encoding="utf-8") as f:
        json.dump(scene, f, indent=2)
        f.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
