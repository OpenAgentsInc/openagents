---
id: tool.freecad-headless-scripting
version: 1
kind: tool
title: Build and verify FreeCAD models headless with freecadcmd and the Part API
summary: >-
  Script FreeCAD without a display through freecadcmd (or by importing the
  FreeCAD module), build solids with the Part API, recompute and save the
  document, and verify the result by measurement: validity, solid count,
  volume, bounding box, and feature dimensions, not by how the script reads.
tags: [freecad, cad, python, part-api, techdraw, step, headless]
applies_when: >-
  Creating, editing, or checking a FreeCAD model or drawing from a script in
  an environment without a GUI.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "FreeCAD Wiki, Topological data scripting (Part module shapes, booleans, fillets)"
    - "FreeCAD Wiki, Part API and TopoShape API (isValid, check, Volume, BoundBox, exportStep)"
    - "FreeCAD Wiki, FreeCAD Command Line Mode (freecadcmd) and Embedding FreeCAD"
    - "FreeCAD Wiki, TechDraw API (App-side functions such as projectToSVG and writeDXFPage)"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**Running headless.** `freecadcmd script.py` (named `FreeCADCmd` on some
installs) runs a script with the application modules and no GUI. From plain
Python, add FreeCAD's `lib` directory to `sys.path` and `import FreeCAD`;
the Python version must match the one FreeCAD was built with. `FreeCADGui` and
the `*Gui` modules are unavailable without a display, so anything that needs a
view provider (screenshots, some drawing exports) must use an App-side
alternative or a virtual display (`xvfb-run`).

**Modeling with Part.** Build shapes directly: `Part.makeBox(l, w, h, base)`,
`Part.makeCylinder(r, h, base, dir)`, `Part.makeCone`, extrusions of wires
(`face.extrude(Vector)`), revolutions, and booleans `a.cut(b)`, `a.fuse(b)`,
`a.common(b)`; fillets with `shape.makeFillet(r, edges)`. Units are
millimetres. Put results into a document so they persist:
`obj = doc.addObject("Part::Feature", "Name"); obj.Shape = shape`, then
`doc.recompute()` and `doc.saveAs(path)`. Parametric PartDesign bodies
(sketches with constraints, Pad, Pocket) also work headless, but check
`obj.isValid()`/`obj.State` after `recompute()` since a failing feature leaves
an error state rather than raising. Export with `shape.exportStep(path)`,
`shape.exportStl(path)`, or `Part.export([obj], path)`.

**Drawings.** TechDraw pages, templates, and views are App objects and can be
created headless, but rendering and some exports are GUI-side. App-side
helpers in the `TechDraw` module (for example projecting a shape to SVG
along a direction, or writing a page as DXF) work without a GUI; check
`dir(TechDraw)` in the installed version before relying on a name. An `.FCStd`
file is a zip archive whose `Document.xml` lists objects and properties, which
is a quick way to inspect what a saved file actually contains.

## How to check

Measure the result rather than trusting the construction: `shape.isValid()`
and `shape.check(True)` (raises on defects), `len(shape.Solids)` (a stray
boolean often leaves 0 or 2 solids), `shape.Volume` against a hand-computed
value, `shape.BoundBox` against the overall dimensions, the radius of
cylindrical faces (`face.Surface.Radius`) for holes, and
`shape.distToShape(other)` for clearances. Reopen the saved file in a new
`freecadcmd` process and repeat the checks, and re-import any exported STEP to
confirm it round-trips to one valid solid.
