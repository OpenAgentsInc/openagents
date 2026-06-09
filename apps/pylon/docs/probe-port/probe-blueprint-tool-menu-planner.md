# Probe Blueprint Tool Menu Planner

Date: 2026-06-07

Status: implemented for Probe issue #175.

Probe now has a backend-independent `ProbeToolMenuPlanner`. The planner takes a
typed Blueprint signature lookup result and backend/assignment facts, then
produces a bounded Probe tool menu that can be projected later into Apple FM,
hosted API tools, Codex-style execution, Psionic/Qwen routes, SHC boxes, Pylons,
or swarm inference.

The menu is intentionally ref-first. Each tool carries `toolRef`, `toolName`,
Program Signature and Program Type refs, Context Pack refs, Source Authority
refs, approval policy ref, evidence requirement refs, receipt requirement refs,
schema refs, and a policy of `allow`, `approval_required`, or `deny`. Context
Pack and Source Authority refs only narrow the menu; they do not widen base
runner authority or expose every Probe tool by default.

Unsupported tool scopes are omitted with structured warnings. Denied tool scopes
are moved into `deniedTools` with structured warnings and are not exposed in the
active tool list. The planner enforces `maxToolCount` and records omitted scopes
as warnings instead of silently changing the menu.

The current tool catalog is intentionally small:

- `tool.probe.read_file`
- `tool.probe.code_search`
- `tool.probe.record_evidence`
- `tool.probe.propose_action_submission`

Apple FM projection remains a later step. The planner output is not
backend-specific and does not execute tools.
