# 204. A sandbox and approvals

> **Status: unimplemented — design draft.** Nothing here runs yet.

A tool that runs a process can do anything the harness process can do, unless
you confine it. This chapter confines every spawned process to declared roots
and resources, and asks for approval before anything outside them. No tool
inherits the host's authority by default.

## What this chapter covers

- Declared roots. A process sees only the directories its grant names.
- Resource limits: time, memory, output bytes and process count.
- An approval policy. A call outside the grant pauses and asks the person, and
  the answer is logged.
- Per-tool timeouts.
- A guard against repeated failure. The harness detects the same failing call
  repeated with the same arguments and stops it.

## Dependencies

- [Chapter 203](ch203-tools-as-typed-capabilities.md): the tool pipeline that the sandbox wraps.
- [Chapter 201](ch201-model-visible-means-logged.md): the log that records grants and approvals.

## Done means

- A process that reads or writes outside its declared roots is refused, and no
  effect occurs outside them.
- A process that exceeds a limit stops, and the step records which limit.
- A call outside the grant does not run until an approval is logged.
- The repeated-failure guard stops a scripted loop of identical failing calls.
- Each check runs with local, disposable files only.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
