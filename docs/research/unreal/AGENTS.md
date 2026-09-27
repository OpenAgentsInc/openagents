# Studying Unreal Engine source

Rules for agents and people who read Unreal Engine source to improve our own
code. Read this before opening any Unreal file, and follow it for every
change that draws on what you read.

This is an engineering policy, not legal advice. It records how we read the
[Unreal Engine EULA](https://www.unrealengine.com/eula/unreal) as of
2026-09-27. If Epic changes those terms, re-read them and update this file.

## What the license allows

- **Reading and private modification.** EULA §2 licenses a User to "privately
  use, reproduce, display, perform, and modify" the Licensed Technology. Reading
  Chaos to learn how it solves a problem falls within that license.
- **Ideas are ours to reuse.** Copyright protects the code's expression. It
  does not protect algorithms, data layouts, tuning ranges, or architecture.
  We may write our own implementation of a technique we learned from Unreal.
- **Short snippets for discussion.** §5(a)(ii) allows public posts of up to
  30 lines, and only to discuss the snippet. Do not use that allowance in this
  repo. It does not cover porting code in slices.

## What it forbids

- **No Unreal code in any of our repos.** `openagents` is public and
  Apache-2.0. Under §4 and §5(a), Engine Code may go only to other Epic
  licensees, through Epic's GitHub fork network or Fab. That rules out copied
  lines, commented-out excerpts, and "reference" files.
- **No transliterations.** A Rust port that follows an Unreal function's
  structure, names, and control flow is a derivative work, whatever the
  language. Our version must come from our own understanding and design.
- **No Unreal content** such as Starter Content, MetaHuman data, or engine
  shaders. Examples (the Samples and Templates folders) may be redistributed
  under §5(b), but ask the owner before relying on that.
- **No AI training input.** Under §6(e), Unreal source must not be training
  input to a generative AI program. It also must not be prompt input to a
  program that trains on its inputs. Before an agent session reads Unreal
  source, confirm that the account or API terms behind it do not train on
  inputs. If you cannot confirm this, do not read Unreal source in that
  session.
- **No combination with GPL-style code** (§6(c)). This matters only if
  Unreal code were linked into something. Our policy of never bringing in
  Unreal code already rules that out.

## Where the source lives

- Keep the clone **outside every tracked tree**. The owner's machine uses
  `~/work/UnrealEngine` (ignored by the workspace root's `.gitignore`) or
  `~/code/UnrealEngine`. Never clone into `projects/` or any child repo.
  Their sync scripts and manifests treat `projects/` as redistributable
  reference material.
- Access requires an Epic account linked to GitHub, which accepts the EULA.
  Only people who have accepted it may read the clone.
- A sparse checkout of the physics sources is enough for the physics work:
  `Engine/Source/Runtime/Experimental/{Chaos,ChaosCore,ChaosVehicles}`,
  `Engine/Source/Runtime/PhysicsCore`, and
  `Engine/Plugins/Experimental/ChaosVehiclesPlugin`.
- Record the release and commit you studied in every research note.

## How to study and port

1. **Read with a question.** Start from a gap in our code: a failing test, a
   missing mechanism, or a roadmap phase. Do not survey for its own sake.
2. **Write a candidate in our own words** in a dated note in this folder, using
   the template below. Describe the technique the way a paper would: the data,
   the steps, and why it is stable or fast. No code, and no pseudo-code that
   mirrors their functions line by line. Identifiers, cvar names, and file
   paths are fine as pointers.
3. **Find the public source.** Most of Chaos implements published work:
   Müller and Macklin's XPBD papers, Catto's soft step, Gregorius and van den
   Bergen on collision, Tonge on joints. Cite that source in the note and in
   code comments. Permissively licensed engines (Box2D v3, Jolt, Rapier,
   PhysX 5, Bullet) may be read and quoted under their own licenses. Prefer
   them when they cover the same technique.
4. **Implement from the note, not from the open file.** Close the Unreal
   source before writing the Rust. Use our naming, our types, and our
   module boundaries.
5. **Prove it with our tests.** Every port lands with a test in the owning
   crate, such as a momentum ledger, trace, or scenario. "Unreal does it this
   way" is not evidence that it works for us.
6. **Never commit excerpts.** Before a commit, check the diff for pasted
   Unreal code or comments copied from Unreal. Commit messages, PR bodies,
   and issues follow the same rule.

## Candidate template

```markdown
### <Short name> — P1|P2|P3, S|M|L

- **Unreal:** `Engine/…/File.cpp`, `IdentifierName` (UE <version>)
- **Technique:** what it does and why, in our own words.
- **Ours today:** `crates/<crate>/src/<file>.rs` — what exists and the gap.
- **Why it matters:** the scene, test, or failure it would fix.
- **Public reference:** paper, talk, or permissive engine to implement from.
- **Status:** candidate | issue #… | landed in <commit> | rejected (reason)
```

Priority: **P1** adopt soon, **P2** later, **P3** note only. Size: **S** under
a day, **M** a few days, **L** a phase of its own.

Keep a candidate's status current. When one becomes work, open an issue and
link it here. When it lands, record the commit. Rejected candidates stay in
the note with their reason.
