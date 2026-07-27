# Omega network sniffer specification

- Status: proposed
- Date: 2026-07-26
- Owner: Omega
- Audience: product, engineering, security, and assurance teams
- Source revision: `OpenAgentsInc/omega@2ba0381f6824f1b6b6f0ed4225ed99adf761f0c3`

## 1. Purpose

Omega must let a user run a third-party external agent and record its network
traffic for local analysis. The record must show which remote systems the
process used. It must also preserve the bytes that moved in each direction.

The first use case is an ACP external agent. Omega starts this agent as a child
process. The agent can then start its own tools. The sniffer must cover the
external agent and its child process tree.

Omega Agent must have a safe tool that can inspect the capture. A user must also
be able to open the capture with normal local data tools.

## 2. Current Omega state

Omega has most of the control path that this feature needs.

1. `crates/agent_servers/src/acp.rs` starts each local external agent as a child
   process. Omega connects to the process through ACP on standard input and
   standard output.
2. `crates/agent_servers/src/custom.rs` adds the configured proxy environment to
   that child process.
3. `crates/agent_servers/src/acp.rs` keeps an in-memory ACP debug log. The log
   contains ACP messages and standard error. It does not contain the network
   traffic of the agent.
4. `crates/acp_tools/src/acp_tools.rs` gives a developer a live ACP log view.
   This view is not a durable network record.
5. `crates/sandbox/src/sandbox.rs` can confine a local command to an in-process
   HTTP and HTTPS proxy on macOS and Linux.
6. `crates/http_proxy` accepts HTTP proxy requests and HTTPS `CONNECT` tunnels.
   It records the host, port, method, policy result, byte totals, and duration.
   It then moves the tunnel bytes without inspection.
7. `crates/acp_thread/src/terminal.rs` uses that sandbox and proxy for terminal
   tools with restricted network access. The external ACP agent launch path does
   not use the same network wrapper.

These facts give Omega a short first path. Omega can reuse the proxy, the
sandbox, the external-agent process owner, and the ACP log view. Omega needs a
durable capture store, a capture lifecycle, and an agent inspection tool.

## 3. Terms

**Capture session** means one durable traffic record for one external-agent
connection generation.

**Connection generation** means one external-agent child process from start
until exit or restart. One process can serve more than one ACP thread.

**Flow** means one client connection through the capture boundary.

**Stream bytes** means the ordered bytes that the proxy reads from or writes to
one side of a flow. Stream bytes do not include Ethernet, IP, or TCP headers.
They do not include TCP retransmissions.

**Packet bytes** means the bytes from an operating-system packet capture point.
Packet bytes can include network headers and retransmissions.

**Inspection mode** means optional protocol decoding. Inspection can expose
credentials, prompts, source code, and tool results.

## 4. Product behavior

The user starts sniffer mode for a selected external agent. Omega must restart
that agent connection before capture starts. This rule makes the first child
process byte part of the capture.

The user interface must show all of these facts before restart:

- the selected external agent
- the local project
- the capture scope
- the capture file location
- the size limit
- whether protocol inspection is off or on
- that one external-agent connection can serve more than one thread

Omega must show a persistent red capture indicator while capture is active. The
indicator must show the current file size. The user can stop capture without
stopping the external agent. Omega must write a final state to the file when
capture stops.

The default mode is `proxy_stream_v1`. It captures HTTP and HTTPS stream bytes
for a local external-agent process tree. HTTPS payload bytes stay encrypted
unless the user separately enables inspection mode.

The capture includes all supported traffic in that process tree. This traffic
can include agent authentication, update checks, model-provider calls, MCP
calls, and external tool calls. Omega cannot isolate external-tool traffic by
name when the external agent does not expose a process identifier. In that
case, the inspection tool uses the ACP tool-call time range and labels the
result as a temporal correlation.

Omega must not label `proxy_stream_v1` as a capture of all packets. The user
interface and the file manifest must use the exact fidelity name.

## 5. Scope and non-goals

### 5.1 First release scope

The first release has this scope:

- local ACP external agents
- the external-agent process and child processes that inherit its environment
- HTTP proxy traffic
- HTTPS `CONNECT` traffic
- WebSocket traffic that uses HTTP or HTTPS proxy setup
- destination DNS resolution that the Omega proxy performs
- allowed and denied connection attempts that reach the proxy
- correlation with ACP sessions and ACP tool-call time ranges
- macOS and Linux enforcement through the existing sandbox

The first release does not claim support for these cases:

- a remote ACP agent on an SSH host or in a remote container
- native Windows external agents
- UDP, QUIC, ICMP, or arbitrary non-proxy TCP protocols
- traffic from a process that escapes the external-agent process tree
- registry package download traffic that occurs before the agent process starts
- exact causal proof that a specific ACP tool call caused a flow

Omega must refuse an enforcing capture when it cannot create the required
sandbox. It must not silently run an uncaptured external agent.

### 5.2 Later packet backend

A later `os_packet_v1` backend can make the full packet claim. It needs a
platform capture boundary that follows the process tree.

- Linux can use a network namespace, a virtual Ethernet pair, and a capture on
  the host side.
- macOS needs an approved Network Extension or another signed system capture
  component that can attribute traffic to the process tree.
- Windows needs a Windows Filtering Platform component. WSL capture only covers
  agents that run inside WSL.

This later backend must write packet bytes to the same capture service. It can
also export PCAPNG. It must keep the fidelity name `os_packet_v1`.

## 6. Architecture

Add a new `network_capture` crate. Use
`[lib] path = "src/network_capture.rs"` in its `Cargo.toml`.

The crate owns these components:

- `CaptureManager` creates, lists, opens, stops, and deletes capture sessions.
- `CaptureWriter` writes flow and byte records to the capture file.
- `CaptureReader` performs bounded queries without loading the full file.
- `CaptureCorrelation` records ACP session and tool-call time ranges.
- `CapturePolicy` holds fidelity, size, retention, and inspection settings.

Extend `http_proxy::ProxyConfig` with an optional capture sink. Each accepted
client connection gets a stable `flow_id`. The proxy sends each byte chunk to
the sink before it forwards that chunk. The record includes direction, time,
stream offset, length, and SHA-256 digest.

The capture sink must not block a proxy connection on the GPUI foreground
thread. It can use a bounded writer channel and a dedicated file thread. A full
channel is a capture failure. Omega must stop the external-agent connection and
finalize the capture as incomplete. It must not drop byte records and continue.

Extend `sandbox::Sandbox` with a capture configuration for restricted network
mode. Keep filesystem access independent from the network rule. The external
agent needs its normal filesystem behavior, but all supported network traffic
must go through the capture proxy.

Extend `agent_servers::AcpConnection::stdio` so it creates the capture boundary
before it starts the child process. Pass the capture proxy variables to the
child. Clear `NO_PROXY` for this process tree. The proxy must reject a request
that targets its own listener. The proxy can permit explicit loopback targets
so local MCP traffic is visible.

The process owner must retain the capture handle until the external agent exits.
It must finalize the file after process exit, restart, capture stop, write
failure, or app shutdown.

## 7. Capture file

Use one SQLite file with the suffix `.omega-net.sqlite3`. Use private file
permissions. Store it below the Omega application data directory, not in the
project.

An active database can use SQLite write-ahead logging. Omega must checkpoint it
when capture stops so the completed artifact is one file.

The minimum schema is:

```sql
CREATE TABLE capture (
  capture_id TEXT PRIMARY KEY,
  schema_version INTEGER NOT NULL,
  fidelity TEXT NOT NULL,
  state TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  connection_generation TEXT NOT NULL,
  omega_version TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  size_limit_bytes INTEGER NOT NULL,
  incomplete_reason TEXT
);

CREATE TABLE flow (
  flow_id TEXT PRIMARY KEY,
  opened_at TEXT NOT NULL,
  closed_at TEXT,
  host TEXT,
  port INTEGER,
  proxy_method TEXT,
  outcome TEXT NOT NULL,
  deny_reason TEXT,
  bytes_to_remote INTEGER NOT NULL,
  bytes_from_remote INTEGER NOT NULL
);

CREATE TABLE byte_chunk (
  sequence INTEGER PRIMARY KEY,
  flow_id TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  monotonic_ns INTEGER NOT NULL,
  direction TEXT NOT NULL,
  stream_offset INTEGER NOT NULL,
  byte_length INTEGER NOT NULL,
  sha256 TEXT NOT NULL,
  bytes BLOB NOT NULL
);

CREATE TABLE correlation (
  correlation_id TEXT PRIMARY KEY,
  flow_id TEXT,
  acp_session_id TEXT,
  tool_call_id TEXT,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  confidence TEXT NOT NULL
);

CREATE TABLE protocol_record (
  record_id TEXT PRIMARY KEY,
  flow_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  sensitive INTEGER NOT NULL,
  value_json TEXT NOT NULL
);
```

The real schema must use foreign keys and indexes. It must index host, time,
flow, direction, and ACP identifiers.

`byte_chunk` is the exact stream record for `proxy_stream_v1`. Chunk boundaries
are observation boundaries. They are not packet boundaries. The digest lets a
reader verify each stored chunk.

The writer must use UTC wall time and a monotonic time for each record. Ordering
between the two directions is an observation order. It is not a remote network
clock order.

The default size limit is 1 GiB. The user can change it before capture starts.
At the limit, Omega must stop the external-agent connection. It must set
`state = 'incomplete'` and `incomplete_reason = 'size_limit'`. It must not keep
the agent online after it stops byte capture.

## 8. HTTPS and sensitive data

Raw HTTPS capture in `proxy_stream_v1` contains the TLS records that cross the
proxy tunnel. It does not show decrypted HTTP requests or responses.

Inspection mode is off by default. It is a separate user grant for one capture
session. A first inspection implementation can use an ephemeral local
certificate authority and a TLS proxy. Omega must give trust only to the child
process environment. It must not add the authority to a system trust store.

Omega must state that some clients use certificate pinning or a private trust
store. These clients can fail or remain opaque in inspection mode. Omega must
record that result.

A capture can contain secrets even when inspection mode is off. Plain HTTP,
destination names, request sizes, and timing can be sensitive. An inspected
capture can contain authorization headers, cookies, prompts, source code, and
tool output.

Therefore:

- Omega must never send a capture to telemetry or Sync.
- Omega must never add a capture to a Git repository.
- Omega must require a user action before it exposes raw or decoded payloads to
  a model.
- Metadata queries must redact URL query values and known credential headers by
  default.
- File export must show a sensitive-data warning.
- Delete must remove the capture and its temporary write-ahead files.

## 9. Omega Agent inspection tool

Add one built-in tool named `inspect_network_capture`. Follow the existing
bounded artifact-reader pattern in
`crates/agent/src/tools/read_tool_result_artifact_tool.rs`.

The input has these fields:

```json
{
  "capture_id": "optional, defaults to the active or latest capture",
  "operation": "summary | list_flows | timeline | search_text | read_bytes",
  "flow_id": "optional",
  "host": "optional",
  "direction": "to_remote | from_remote | optional",
  "after_sequence": 0,
  "limit": 100,
  "text": "required only for search_text",
  "encoding": "utf8 | hex | base64",
  "include_sensitive": false
}
```

The tool returns structured JSON. The output includes the fidelity, capture
state, applied filters, result count, next cursor, and truncation state.

`summary`, `list_flows`, and the metadata part of `timeline` do not need access
to payload data. `search_text` and `read_bytes` need a user grant when
`include_sensitive` is true. The tool must cap one result at 64 KiB. It must use
cursors for more data.

The tool must make these analyses easy:

- list every destination and byte total
- find traffic during one ACP tool-call time range
- show new destinations after a selected time
- compare sent and received byte totals
- search decoded plain-text traffic
- read a bounded byte range as hexadecimal or Base64
- identify denied requests and incomplete captures

Temporal correlation is not causal proof. The output must show
`confidence = 'temporal'` unless Omega has a verified process or protocol
identifier that proves the link.

The ACP log view can add a **Network** tab. It can use the same reader service.
It must not maintain a second capture store.

## 10. Settings

Add a top-level `network_sniffer` settings object with these fields:

```json
{
  "network_sniffer": {
    "default_size_limit_bytes": 1073741824,
    "retention_days": 7,
    "inspection_enabled_by_default": false,
    "capture_directory": null
  }
}
```

`inspection_enabled_by_default` must remain `false` in the shipped defaults.
A custom capture directory must resolve to a local path. Omega must reject a
remote project path.

The user can override retention. Automatic deletion must only delete completed
captures that are older than the limit. It must not delete an active capture.

## 11. Failure behavior

The feature is fail-closed for capture completeness.

Omega must stop the external-agent connection when any of these events occurs:

- the capture proxy cannot start
- the sandbox cannot enforce the proxy boundary
- Omega cannot create the capture file
- the writer channel is full
- a byte write or database commit fails
- the file reaches its size limit

Omega must preserve an incomplete file when possible. The manifest row must
give the exact reason. The user interface and the inspection tool must show the
same reason.

An app crash can lose the current uncommitted writer batch. On the next start,
Omega must run SQLite recovery and set `state = 'incomplete'` with
`incomplete_reason = 'unclean_shutdown'`.

## 12. Delivery plan

### OMEGA-SNIFF-01: Capture store

Add the `network_capture` crate, the SQLite schema, bounded reader APIs, file
permissions, recovery, and size enforcement.

### OMEGA-SNIFF-02: Proxy byte events

Add stable flow identifiers and exact bidirectional chunk events to
`http_proxy`. Keep the current policy events.

### OMEGA-SNIFF-03: External-agent launch boundary

Apply the network-only sandbox and capture proxy before
`AcpConnection::stdio` starts a local child process. Cover process exit and
restart.

### OMEGA-SNIFF-04: User controls

Add start, stop, status, file location, file size, restart notice, incomplete
state, and retention controls.

### OMEGA-SNIFF-05: Agent inspection

Add `inspect_network_capture` and the Network tab in the ACP log view. Add
sensitive-payload permission checks.

### OMEGA-SNIFF-06: Optional TLS inspection

Add the ephemeral child-only trust path and decoded protocol records. Keep this
work separate from raw stream capture.

### OMEGA-SNIFF-07: OS packet backends

Add platform process-tree capture and PCAPNG export. Do not change the fidelity
claim of older files.

## 13. Acceptance tests

The first release is complete when all of these tests pass:

1. A local fixture ACP agent makes one plain HTTP request and one HTTPS request.
   The capture has both destinations and both directions.
2. The stored plain HTTP bytes match the fixture bytes exactly.
3. The stored HTTPS tunnel bytes match the bytes that the proxy moved. The
   capture does not claim decrypted HTTP content.
4. A child process of the fixture agent makes a request. The same capture has
   that flow.
5. A direct network attempt cannot bypass the proxy on macOS and Linux.
6. A proxy or writer failure stops the external-agent connection and marks the
   capture incomplete.
7. A size-limit event stops the connection without a silent byte gap.
8. An app restart recovers an unclean file and marks it incomplete.
9. `inspect_network_capture` can list destinations and read a bounded byte
   range.
10. The tool cannot read sensitive payload data without the required user
    grant.
11. No capture file enters Sync, telemetry, or the project worktree.
12. The capture indicator stays visible for the complete active interval.
13. A remote external agent gets a clear unsupported result. Omega does not
    create an empty file and call it complete.

## 14. Open decisions

These decisions need implementation research before `OMEGA-SNIFF-06` or
`OMEGA-SNIFF-07` starts:

- the signed macOS Network Extension type and entitlement path
- the Windows Filtering Platform distribution and update path
- whether decrypted protocol records store full bodies or selected fields
- whether an inspected capture can use a separate encryption key
- the PCAPNG interface model for a process-tree capture

They do not block `proxy_stream_v1`.
