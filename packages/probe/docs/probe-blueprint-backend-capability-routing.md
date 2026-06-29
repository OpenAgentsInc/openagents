# Probe Blueprint Backend Capability Routing

Date: 2026-06-07

Status: implemented for Probe issue #179.

Probe backend capability reports now include a `blueprintSupport` projection
alongside live Apple FM health. This lets Pylon, SHC, sandbox, and local
runners advertise whether they can run a Blueprint Program Signature/tool menu
on a concrete backend, rather than only saying that the raw backend is present.

The report remains a public/operator-safe projection. It carries refs and
capability facts only:

- supported Blueprint registry version refs
- safe projection policy refs
- supported Program families, Program Type refs, Program Signature refs, and
  Module Version refs
- supported Probe tool refs
- backend tool projection adapter refs
- Apple FM schema projection support and max projected tool count
- whether local Program Run evidence can be recorded offline
- local, swarm, and API backend availability facts for route selection

Ready Apple FM health is necessary but not sufficient for route eligibility.
Probe advertises runnable capabilities only when Apple FM health is ready, the
Blueprint registry projection is safe, and the Apple FM tool schema projection
is supported. If the registry slice or projection support is malformed, the
report still exposes operator-visible health and redacted warnings, but
`available` is false and `advertisedCapabilities` is empty.

Pylon and SHC routing should use these facts as narrowing inputs. They can
select runners whose advertised registry version, Program Signature refs, tool
refs, projection adapter, and backend availability match the assignment. They
must not use capability reports to widen assignment authority, source
authority, context scope, or action policy. The assignment's Blueprint section
and runner proof remain the authority boundary.

Capability reports must not include raw prompts, raw tool schemas containing
private context, provider credentials, callback URLs, callback tokens, wallet
material, private repo contents, or raw local logs. Tests cover redaction,
ready Apple FM support, unsupported Apple FM health, malformed Blueprint
registry support, and malformed Apple FM projection support.
