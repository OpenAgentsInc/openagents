# 35. Bring in robots and meshes

> **Status: unimplemented — design draft.** Psionic's Panda work reads a
> robot description through MuJoCo; this chapter plans our own importers.
> Nothing here runs yet.

Nobody writes a robot's inertias by hand. They arrive as URDF or MJCF files
from the manufacturer or a model zoo, with meshes for appearance and
collision. Importing is where most simulation errors are born: a mesh in
millimetres, an inertia tensor that is not positive definite, a collision
mesh that is not watertight. The importer's job is to turn these files into
checked scene data, and to refuse what it cannot check.

## What this chapter teaches

- URDF and MJCF: links, joints, inertias, limits, actuators and the
  differences between them.
- Meshes: OBJ, STL, glTF; units, winding and watertightness.
- Convex decomposition for collision and tetrahedralization for FEM.
- Physical plausibility checks on inertias.

## The checks

An inertia tensor `I` about the centre of mass must be symmetric positive
definite and satisfy the triangle inequalities on its principal moments:

```jai
I₁ + I₂ ≥ I₃,  I₂ + I₃ ≥ I₁,  I₁ + I₃ ≥ I₂,   I = Iᵀ ≻ 0
```

A collision mesh must be closed and consistently oriented (every edge
shared by exactly two faces in opposite directions) before it can bound a
volume. Convex decomposition splits a non-convex mesh into convex hulls
(approximate convex decomposition, [Wei et al., CoACD, SIGGRAPH 2022](https://doi.org/10.1145/3528223.3530103));
tetrahedralization fills a closed surface with quality tetrahedra
([Hu et al., TetWild, SIGGRAPH 2018](https://doi.org/10.1145/3197517.3201353)).

## The design in Aethyr

Coax R17's robot-model import is the language half: parsing into checked
records with units and frames. Aethyr adds the physical checks above, mesh
validation, decomposition and tetrahedralization, and writes a content-
addressed scene asset that build (chapter 17) reads, so a run names the
exact asset bytes it used. Each imported field records provenance (R20):
measured, from the vendor file, or inferred (for example, inertia computed
from a mesh and a density). MJCF features we do not support are refused by
name, not ignored. USD is later.

## What it builds

- `aethyr/sim/import/`: URDF and MJCF readers, mesh readers,
  validators, decomposition and tetrahedralization, asset writer.
- The Panda and a quadruped imported and checked.

## Acceptance checks and oracles

- Imported Panda's kinematic tree, limits and inertias equal values
  recorded from MuJoCo's loader of the same file.
- Inertia and mesh validators refuse each fixture of a corpus of broken
  files with its named reason.
- Decomposed hulls contain the original surface to a stated distance;
  tetrahedral volume equals the enclosed volume within tolerance.
- The asset digest is stable across imports.

## How Genesis does it

Genesis imports URDF, MJCF, meshes and USD, with convex decomposition and
tetrahedralization for collision and FEM
([morphs](https://genesis-world.readthedocs.io/en/latest/api_reference/options/morph/index.html)).
We plan the same formats except USD at first, with physical validation and
provenance as part of import.

## Dependencies

Chapter 17 defines the scene asset boundary, and chapter 19 defines the
link/joint records. Importing, validating and hashing those records does not
require executing the solvers that later consume them. Coax supplies R12
units and frames, R17 robot-model import and R20 provenance.

The rigid and material solvers, batch resets, gradients, sensors and
observation renderer of chapters 18–34 are downstream integrations. They
must check imported assets against their authored fixtures before claiming
the imported profile. In particular, chapter 21 adds baked/decomposed
geometry, chapter 27 adds generated tetrahedra, and chapter 34 needs the
admitted meshes and textures for its observation images.
