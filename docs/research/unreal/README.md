# Unreal Engine research

Notes from studying Unreal Engine source to decide what to reimplement in our
own code. Unreal's code is proprietary: we reuse its ideas, never its code.
Read [AGENTS.md](AGENTS.md) before opening any Unreal file or adding a note
here. It covers what the EULA allows, where the clone lives, and how to go
from reading to a tested port.

| Note | Scope |
| --- | --- |
| [2026-09-27-chaos-physics-candidates.md](2026-09-27-chaos-physics-candidates.md) | Chaos (UE 5.8.3) against `crates/physics`: solver, collision, joints, determinism, and tooling candidates, ranked, plus rejections |

When a candidate becomes an issue or lands, update its **Status** in the
note. Put new notes here with a date prefix and add a row to this table.
