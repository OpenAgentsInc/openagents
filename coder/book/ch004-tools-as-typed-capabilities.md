# 4. Tools as typed capabilities

A tool is not an arbitrary string handed to a shell; it is a typed capability
with fail-closed defaults, explicit schema fields, deterministic ordering, and
anti-shadowing boundaries.

In OpenAgents, tool contracts and pool assembly live in
[`openagents/core/tool.coax`](../../openagents/core/tool.coax), concurrency
batching lives in [`openagents/core/tool_exec.coax`](../../openagents/core/tool_exec.coax),
and desktop/member capability packs (`OA_PACK_COAX_INTEL`, `OA_PACK_WALLET`,
`OA_PACK_CLI`, `OA_PACK_METAVERSE`, `OA_PACK_AGENTS`, `OA_PACK_KNOWLEDGE`,
`OA_PACK_CAPABILITY`, `OA_PACK_SESSIONS`) are governed by
[`openagents/core/members.coax`](../../openagents/core/members.coax). All three
compile into [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax).

## Profile

This chapter's profile is `openagents-tool-capabilities-v1, bounded CPU; coder-owned-v1, native Linux x86_64`:

- **Fail-closed defaults (`flawed_tool_defaults`):** every new tool starts with
  `read_only = false`, `concurrency_safe = false`, and
  `interrupt_behavior = FLAWED_INTERRUPT_BLOCK`. Safety properties must be
  opted into explicitly.
- **UNC path defense (`flawed_is_unc_path`):** rejects SMB/UNC paths
  (`\\host\share` and `//host/share`) before filesystem calls can leak NTLM
  credentials.
- **Partitioned cache-stable pool + anti-shadowing (`flawed_assemble_tool_pool`):**
  built-in tools are sorted into the leading partition ahead of MCP tools (so
  MCP churn never shifts built-in schemas in the prompt cache), blanket deny
  rules such as `mcp__danger` strip matching server prefixes, and an MCP server
  can never shadow a built-in tool name like `Read`.
- **Fail-closed concurrency partitioning (`flawed_partition_tool_calls`):**
  contiguous concurrency-safe valid calls group into parallel batches while any
  mutating or invalid call forms its own serial barrier.
- **Closed member-pack CLI allowlist (`oa_cli_subcmd_allowed`, `oa_cli_arg_safe`):**
  `OpenAgentsCli` accepts only its 8 closed subcommands (`nostr`, `pylon`,
  `project`, `dictate`, `transcribe`, `realm`, `vault`, `tasks`) and rejects
  control characters, NUL bytes, and newlines before `argv` execution.

## Worked example

The runnable example [tool-capabilities.coax](../examples/tool-capabilities.coax)
exercises fail-closed defaults, UNC rejection, blanket MCP deny rules,
anti-shadowing pool assembly, concurrency partitioning, and member-pack CLI
validation directly against `openagents/core/`.

<!-- source: coder/examples/tool-capabilities.coax example -->
```jai
main :: () {
  // 1. Fail-closed defaults: new tools default to mutating, non-concurrent, non-read-only
  edit_arr: [1] FlawedToolSpec;
  edit_arr[0] = flawed_tool_defaults("Edit", FLAWED_TOOL_SRC_BUILTIN);
  _ = flawed_tool_add_field(edit_arr[..], "path", FLAWED_FIELD_STRING, true);
  edit := edit_arr[0];
  read := flawed_tool_defaults("Read", FLAWED_TOOL_SRC_BUILTIN);
  read.read_only = true;
  read.concurrency_safe = true;
  print("defaults: Edit(ro=%,conc=%) Read(ro=%,conc=%)\n",
    edit.read_only, edit.concurrency_safe, read.read_only, read.concurrency_safe);

  // 2. UNC path guard blocks SMB credential leak paths (\\host\share and //host/share)
  print("unc guard: smb_bs=% smb_fs=% posix=%\n",
    flawed_is_unc_path("\\\\attacker\\share\\a.txt"),
    flawed_is_unc_path("//attacker/share/a.txt"),
    flawed_is_unc_path("/repo/coder/book/README.md"));

  // 3. Blanket deny rule ("mcp__danger") blocks mcp__danger__exec while keeping mcp__safe__list
  //    and built-ins cannot be shadowed by an MCP tool with the same name.
  pctx: [1] FlawedToolPermContext;
  _ = flawed_perm_ctx_init(pctx[..], OA_MP_PMODE_DEFAULT, false);
  _ = flawed_perm_add_deny(pctx[..], "mcp__danger");

  bi: [2] FlawedToolSpec;
  bi[0] = read;
  bi[1] = edit;
  mcps: [3] FlawedToolSpec;
  mcps[0] = flawed_tool_defaults("mcp__danger__exec", FLAWED_TOOL_SRC_MCP);
  mcps[1] = flawed_tool_defaults("mcp__safe__list", FLAWED_TOOL_SRC_MCP);
  mcps[2] = flawed_tool_defaults("Read", FLAWED_TOOL_SRC_MCP); // shadow attempt

  pool: [1] FlawedToolPool;
  _ = flawed_assemble_tool_pool(pool[..], pctx[..], bi[..], mcps[..]);
  print("pool assembled: count=% built_in_edit_idx=% mcp_safe_idx=% mcp_danger_idx=%\n",
    pool[0].count,
    flawed_pool_find(pool[..], "Edit"),
    flawed_pool_find(pool[..], "mcp__safe__list"),
    flawed_pool_find(pool[..], "mcp__danger__exec"));

  // 4. Concurrency batching: [Read, Read, Edit, Read] -> [conc(2), serial(1), conc(1)]
  calls: [4] FlawedBatchCall;
  calls[0] = FlawedBatchCall.{id = 1, valid_input = true, concurrency_safe = true};
  calls[1] = FlawedBatchCall.{id = 2, valid_input = true, concurrency_safe = true};
  calls[2] = FlawedBatchCall.{id = 3, valid_input = true, concurrency_safe = false};
  calls[3] = FlawedBatchCall.{id = 4, valid_input = true, concurrency_safe = true};
  batches: [4] FlawedExecBatch;
  nb := flawed_partition_tool_calls(calls[..], 4, batches[..]);
  print("batches=%: b0(conc=%,len=%) b1(conc=%,len=%) b2(conc=%,len=%)\n",
    nb, batches[0].is_concurrent, batches[0].count,
    batches[1].is_concurrent, batches[1].count,
    batches[2].is_concurrent, batches[2].count);

  // 5. Member pack closed CLI allowlist & argument sanitization
  packs := oa_member_packs_from_flags(true, true, true, false);
  print("member packs mask=% cli(nostr)=% cli(rm)=% arg_newline_safe=%\n",
    packs, oa_cli_subcmd_allowed("nostr"), oa_cli_subcmd_allowed("rm"), oa_cli_arg_safe("status\nrm -rf /"));
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/tool-capabilities.coax
```

```text
defaults: Edit(ro=false,conc=false) Read(ro=true,conc=true)
unc guard: smb_bs=true smb_fs=true posix=false
pool assembled: count=3 built_in_edit_idx=0 mcp_safe_idx=2 mcp_danger_idx=-1
batches=3: b0(conc=true,len=2) b1(conc=false,len=1) b2(conc=true,len=1)
member packs mask=7 cli(nostr)=true cli(rm)=false arg_newline_safe=false
```

The checked fixture is [tool-capabilities.out](../examples/tool-capabilities.out).

## Checks and production adoption

- `#authority` unit and mutation suites in
  [`openagents/core/tool.coax`](../../openagents/core/tool.coax),
  [`openagents/core/tool_exec.coax`](../../openagents/core/tool_exec.coax), and
  [`openagents/core/members.coax`](../../openagents/core/members.coax).
- Adopted live in [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax)
  and the native Linux journaled worker
  ([`coax/lib/agent/native-profile.coax`](../../coax/lib/agent/native-profile.coax)).

## Done means

- [x] Tool defaults fail closed, UNC paths are blocked, MCP tools cannot shadow
  built-ins, and concurrency batching isolates serial calls under `#authority`.
- [x] `coder/examples/tool-capabilities.coax` matches
  `coder/examples/tool-capabilities.out` in debug and release.
- [x] `openagents/runtime/oa-worker.coax` mounts built-in and member packs through the
  shared `#authority` policy.
