# Program authority

A selected program runs only when the operator grants its slug. This applies to
both the terminal and `coder -p`, through the shared turn and runtime.

```sh
export CODER_PROGRAMS=burn-down,project-task
export CODER_PROGRAM_EFFECTS=reads,writes,delegation,network,subprocesses,spend
coder --programs burn-down -p 'Run the burn-down program for the prepared work list.'
```

An unset `CODER_PROGRAMS` grants no programs. `--programs` adds named slugs to the
environment's list; it does not replace it. `all` grants every resolvable program.
Use named slugs for a scoped supervisor. The environment's `CODER_PROGRAM_EFFECTS`
is a ceiling: a CLI program grant cannot widen it. An unset effect ceiling allows
all effects for a program whose slug is authorized; it does not authorize a slug.
To withdraw all program authority, remove CLI grants and set `CODER_PROGRAMS=none`.

The runtime derives effects from step behavior and executor metadata, refuses
missing authority before running steps, and checks each selected work item's
write requirement before delegation. A work list, model judgment, or successful
independence answer cannot grant authority. The ATIF `program_authority` call
records the grant and derived effects. `CODER_SHELL=off` still controls the
ordinary generated-command loop separately.

The six effect names are `reads`, `writes`, `delegation`, `network`,
`subprocesses`, and `spend`. These authorize effects; they are not resource or
isolation guarantees. The existing filesystem boundary confines writes. It does
not confine reads or network access. Unsupported step bounds, including read-path
or network allowlists and hard monetary limits, are refused. Do not describe an
allowlist in a prompt as enforcement.

Capability approval is revalidated by the existing executor boundary. Runtime
program composition remains unsupported and refused; `Grant::meets` implements
the narrowing operation future composition must use. Dynamic Decision Router
tenant/model authorization is not implemented by this local grant. It remains
part of [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502), the
shared client integration. The local host-authority foundation in #9504 is complete.

Tests cover missing grants, effect ceilings, child-grant intersection, unsupported
bounds, a spurious program selection from bullet prose, and writing work-list
data under a grant without writes. The
[Devin runbook](devin-delegation-runbook.md) includes the explicit grant needed by
its previously authorized delegation procedure.
