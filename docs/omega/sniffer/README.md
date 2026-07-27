# Omega network sniffer specification

- Status: proposed
- Date: 2026-07-26
- Owner: Omega
- Audience: product, engineering, security, and assurance teams
- Source revision: `OpenAgentsInc/omega@2ba0381f6824f1b6b6f0ed4225ed99adf761f0c3`

## 1. Purpose

Omega must let a user select any local application and record its network
traffic for local analysis. The application does not connect to Omega. It does
not need an Omega plugin, ACP support, a proxy setting, or an environment
variable.

The selected application can already be open before Omega starts capture. Omega
must attach to the verified running process and follow its child process tree.
Omega must also support a launch-under-capture action when the user needs the
first process byte.

The record must show which remote systems the application used. It must also
preserve the bytes that moved in each direction. The operating-system capture
boundary must provide the traffic to Omega.

An ACP external agent that Omega starts is an additional use case. It is not the
required integration boundary.

Omega Agent must have a safe tool that can inspect the capture. A user must also
be able to open the capture with normal local data tools.

## 2. Current Omega state

Omega has part of the control and inspection pattern that this feature needs.
It does not have the operating-system capture boundary.

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

These facts give Omega a short path for an application that Omega starts. They
do not capture an independent application that ignores proxy settings.

The independent-application requirement needs an operating-system capture
component. Omega also needs a durable capture store, a target identity model, a
capture lifecycle, and an agent inspection tool.

## 3. Terms

**Capture session** means one durable traffic record for one verified
application target.

**Application identity** means the stable platform identity, signer identity,
and executable identity that Omega verifies before capture.

**Process instance** means a process ID plus an operating-system process-start
identity. The operating system can reuse a process ID, so the ID is not
sufficient.

**Target process tree** means the selected process, its existing children at
attachment time, and the new children that it creates during capture.

**Flow** means one client connection through the capture boundary.

**Stream bytes** means the ordered bytes that the proxy reads from or writes to
one side of a flow. Stream bytes do not include Ethernet, IP, or TCP headers.
They do not include TCP retransmissions.

**Packet bytes** means the bytes from an operating-system packet capture point.
Packet bytes can include network headers and retransmissions.

**Inspection mode** means optional protocol decoding. Inspection can expose
credentials, prompts, source code, and tool results.

## 4. Product behavior

The sniffer starts with an application picker. The picker lists running
applications and processes. It can show a friendly name and an icon, but Omega
must bind capture to a verified identity.

The user can select one of these actions:

- **Attach now** starts capture for an application that is already open.
- **Launch under capture** starts the application after the capture boundary is
  ready.

Attach mode cannot recover traffic that occurred before attachment. Omega must
show the attachment time in the interface and in the capture file.

The user interface must show all of these facts before capture:

- the application name
- the stable application identity
- the signer and executable identity
- the current process instances
- whether Omega will include child processes
- the capture scope
- the capture file location
- the size limit
- whether protocol inspection is off or on

Omega must show a persistent red capture indicator while capture is active. The
indicator must show the current file size. The user can stop capture without
stopping the application. Omega must write a final state to the file when
capture stops.

The required mode is `os_app_flow_v1`. It captures TCP stream bytes and UDP
datagrams that the operating system attributes to the selected application.
This includes QUIC traffic as UDP datagrams. HTTPS and QUIC payload bytes stay
encrypted unless the user separately enables inspection mode.

The capture includes the supported traffic of the target process tree. This can
include application authentication, update checks, model-provider calls, MCP
calls, and external tool calls. The application does not need to identify these
operations to Omega.

Omega can keep `proxy_stream_v1` as a narrower mode for an external agent that
Omega starts. This mode is not completion of the independent-application
requirement. The user interface and file manifest must always show the exact
fidelity name.

### 4.1 Application selection and identity

A display name is not a capture identity. Omega must use the platform identity
and bind it to the current process instance.

The picker can search by display name, platform application identifier,
executable path, or process ID. Search produces candidates only. The user must
confirm the resolved identity before capture.

On macOS, use these values when they are available:

- bundle identifier
- Team ID and designated code requirement
- canonical executable path and code-directory hash
- process ID and audit token

For an unsigned macOS command, use the canonical executable identity and the
process instance. Do not invent a bundle identifier.

On Windows, use these values when they are available:

- package family name and AppUserModelID
- canonical executable path and Authenticode signer
- executable hash
- process ID and creation time

On Linux, use these values when they are available:

- desktop file identifier as a display and selection hint
- canonical `/proc/<pid>/exe` device and inode identity
- executable hash
- process ID, boot identifier, and process start time

Omega must show all matching process instances when an identity has more than
one running instance. The user can capture all matching instances or select
specific instances. Omega must not guess from the process name.

A saved application target is a convenience, not permanent authority. Omega
must verify the signer and executable identity each time. If the identity
changed, Omega must ask the user to select the target again.

The target process tree includes children that existed before attachment and
children that start later. A shared helper outside that ancestry is not part of
the target. The user can add a helper only as another explicit verified target.

## 5. Scope and non-goals

### 5.1 Required independent-application scope

The product requirement has this scope:

- an independent local application with no Omega integration
- attach to a running process
- launch under capture
- verified application and process identity
- existing and new child processes
- TCP, UDP, QUIC, and DNS traffic that the operating system can attribute to the
  target
- stream bytes and datagram bytes in both directions
- destination addresses, ports, byte totals, and time
- macOS as the first supported platform
- Linux and Windows backends with the same capture contract

The product requirement does not claim support for these cases:

- traffic that occurred before attach mode started
- a remote application on an SSH host or in a remote container
- traffic from a process outside the verified target process tree
- a shared system service when the operating system cannot attribute its work
  to the target
- exact application attribution when flow data has no verified process or
  socket relation

Omega must record an attribution level for each flow. Use `verified_process`
when the platform proves the process or socket relation. Use `inferred` only
when a documented platform relation supports the result. Do not include
unattributed flow data in an application-specific result as if attribution were
verified.

### 5.2 Platform capture backends

`os_app_flow_v1` needs a platform capture boundary that follows process
identity without cooperation from the application.

- macOS needs an approved Network Extension or signed system capture component.
  It must use the source application audit identity for flow attribution.
- Linux can use eBPF socket and process attribution with flow capture at the
  applicable network hooks. A launch-under-capture path can also use a network
  namespace.
- Windows needs Windows Filtering Platform filters and a signed capture
  component that binds traffic to the verified application identity.

These components can require one operating-system authorization or component
installation. Omega must explain the request before the system prompt. A
capture component must not broaden capture beyond the selected targets.

The backend must write TCP stream bytes and UDP datagrams to the capture
service. It must keep the fidelity name `os_app_flow_v1`.

A platform can also add `os_packet_v1` when it can preserve network headers and
packet boundaries. That stronger mode can support PCAPNG export. Omega must not
label application flow bytes as packets.

## 6. Architecture

Add a new `network_capture` crate. Use
`[lib] path = "src/network_capture.rs"` in its `Cargo.toml`.

The crate owns these components:

- `CaptureManager` creates, lists, opens, stops, and deletes capture sessions.
- `TargetResolver` lists candidates and verifies application identities.
- `ProcessTreeTracker` follows existing and new target process instances.
- `PlatformCapture` provides the macOS, Linux, or Windows capture backend.
- `CaptureWriter` writes flow and byte records to the capture file.
- `CaptureReader` performs bounded queries without loading the full file.
- `CaptureCorrelation` records process, socket, protocol, and optional ACP time
  relations.
- `CapturePolicy` holds fidelity, size, retention, and inspection settings.

`TargetResolver` must return an opaque verified target. Other components must
not repeat a name or path lookup after verification. `ProcessTreeTracker` must
protect against process ID reuse.

`PlatformCapture` must start before attach or launch completes. It must filter
on verified target identities in the operating-system component. Filtering only
after unrelated flow data reaches the Omega process can expose other
applications and is not acceptable.

The macOS backend is the first delivery target. It must use application audit
identity from the selected Network Extension or system capture API. The
implementation must prove that a same-name application with a different signer
does not enter the capture.

Extend `http_proxy::ProxyConfig` with an optional capture sink. Each accepted
client connection gets a stable `flow_id`. The proxy sends each byte chunk to
the sink before it forwards that chunk. The record includes direction, time,
stream offset, length, and SHA-256 digest.

This proxy extension supports `proxy_stream_v1` for an Omega-launched external
agent. It is a secondary backend. An independent application can ignore proxy
settings, so the proxy cannot satisfy `os_app_flow_v1`.

The capture sink must not block a proxy connection on the GPUI foreground
thread. It can use a bounded writer channel and a dedicated file thread. A full
channel is a capture failure. Omega must stop capture and finalize the file as
incomplete. It must not drop byte records and continue.

Extend `sandbox::Sandbox` with a capture configuration for restricted network
mode. Keep filesystem access independent from the network rule. The external
agent needs its normal filesystem behavior, but all supported network traffic
must go through the capture proxy.

Extend `agent_servers::AcpConnection::stdio` so it creates the capture boundary
before it starts the child process. Pass the capture proxy variables to the
child. Clear `NO_PROXY` for this process tree. The proxy must reject a request
that targets its own listener. The proxy can permit explicit loopback targets
so local MCP traffic is visible.

The capture manager must retain the capture handle until capture stops. It must
finalize the file after target exit, capture stop, write failure, component
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
  attach_mode TEXT NOT NULL,
  target_kind TEXT NOT NULL,
  target_identity_json TEXT NOT NULL,
  omega_version TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  size_limit_bytes INTEGER NOT NULL,
  incomplete_reason TEXT
);

CREATE TABLE process_instance (
  process_instance_id TEXT PRIMARY KEY,
  capture_id TEXT NOT NULL,
  parent_process_instance_id TEXT,
  platform_process_id INTEGER NOT NULL,
  process_start_identity TEXT NOT NULL,
  executable_identity_json TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT,
  relation TEXT NOT NULL
);

CREATE TABLE flow (
  flow_id TEXT PRIMARY KEY,
  process_instance_id TEXT,
  opened_at TEXT NOT NULL,
  closed_at TEXT,
  host TEXT,
  port INTEGER,
  transport TEXT NOT NULL,
  proxy_method TEXT,
  attribution TEXT NOT NULL,
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
  unit_kind TEXT NOT NULL,
  datagram_id TEXT,
  stream_offset INTEGER NOT NULL,
  byte_length INTEGER NOT NULL,
  sha256 TEXT NOT NULL,
  bytes BLOB NOT NULL
);

CREATE TABLE packet (
  sequence INTEGER PRIMARY KEY,
  flow_id TEXT,
  process_instance_id TEXT,
  observed_at TEXT NOT NULL,
  monotonic_ns INTEGER NOT NULL,
  interface_identity TEXT NOT NULL,
  link_type INTEGER NOT NULL,
  direction TEXT NOT NULL,
  packet_length INTEGER NOT NULL,
  captured_length INTEGER NOT NULL,
  attribution TEXT NOT NULL,
  bytes BLOB NOT NULL
);

CREATE TABLE correlation (
  correlation_id TEXT PRIMARY KEY,
  flow_id TEXT,
  source_kind TEXT NOT NULL,
  source_id TEXT NOT NULL,
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
flow, direction, process identity, application identity, and optional ACP
identifiers.

`byte_chunk` is the exact flow record for `os_app_flow_v1` and
`proxy_stream_v1`. Stream chunk boundaries are observation boundaries. UDP
datagram boundaries are protocol boundaries and must stay intact. Neither kind
is an IP packet boundary. The digest lets a reader verify each stored unit.

For `os_packet_v1`, the packet table stores the captured packet bytes, interface
identity, link type, packet length, captured length, direction, flow relation,
process relation, and attribution. The PCAPNG export must preserve the stored
packet order and interface metadata.

The writer must use UTC wall time and a monotonic time for each record. Ordering
between the two directions is an observation order. It is not a remote network
clock order.

The default size limit is 1 GiB. The user can change it before capture starts.
At the limit, Omega must stop capture. It must set `state = 'incomplete'` and
`incomplete_reason = 'size_limit'`. It does not stop an independently owned
application.

## 8. HTTPS and sensitive data

Raw HTTPS and QUIC flow capture in `os_app_flow_v1` contains encrypted protocol
records. It does not show decrypted HTTP requests or responses.

Raw HTTPS capture in `proxy_stream_v1` contains the TLS records that cross the
proxy tunnel. It has the same decryption limit.

Inspection mode is off by default. It is a separate user grant for one capture
session. Omega cannot add an ephemeral certificate authority to an independent
application without changing that application or its system trust. Therefore,
TLS interception is not part of attach mode by default.

Launch-under-capture can offer a child-only trust path for applications that
support one. Omega must not add the authority to a system trust store. Attach
mode can also import supported TLS key-log data when the application already
produces it. Packet capture must remain useful when no decryption path exists.

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
  "operation": "summary | list_targets | list_processes | list_flows | timeline | search_text | read_bytes",
  "process_instance_id": "optional",
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

- show the verified application and process identities
- show which child process owns a flow
- list every destination and byte total
- find traffic during one ACP tool-call time range
- find traffic during any selected time range
- show new destinations after a selected time
- compare sent and received byte totals
- search decoded plain-text traffic
- read a bounded byte range as hexadecimal or Base64
- identify denied requests and incomplete captures

Temporal correlation is not causal proof. The output must distinguish
`verified_process`, `verified_socket`, `inferred`, and `temporal`. It must not
upgrade a temporal relation because the host name looks related to the
application.

Omega must add a top-level **Network Captures** view for independent
applications. The ACP log view can also link to a related capture. Both views
must use the same reader service and capture store.

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

The feature is fail-closed for capture completeness. It is not the owner of an
independent application.

Omega must refuse to start capture when any of these events occurs:

- Omega cannot verify the selected application identity
- the operating-system capture component is absent or unauthorized
- the platform cannot install the target filter
- Omega cannot create the capture file

Omega must stop capture when any of these events occurs:

- the target filter reports an identity mismatch
- the operating-system capture component fails
- the writer channel is full
- a byte write or database commit fails
- the file reaches its size limit

Omega must preserve an incomplete file when possible. The manifest row must
give the exact reason. The user interface and the inspection tool must show the
same reason.

Omega must not stop or modify an application that it attached to. If the user
selected launch-under-capture, Omega must not launch the application when the
capture boundary fails to start.

An app crash can lose the current uncommitted writer batch. On the next start,
Omega must run SQLite recovery and set `state = 'incomplete'` with
`incomplete_reason = 'unclean_shutdown'`.

## 12. Delivery plan

### OMEGA-SNIFF-01: Capture store

Add the `network_capture` crate, the generalized SQLite schema, bounded reader
APIs, file permissions, recovery, and size enforcement.

### OMEGA-SNIFF-02: Target resolver

Add the application picker, platform identity records, signer and executable
verification, process-start identity, and saved-target revalidation.

### OMEGA-SNIFF-03: macOS application capture

Add the signed macOS capture component, audit-identity target filters, TCP and
UDP flow capture, and same-name negative tests.

### OMEGA-SNIFF-04: Process tree and user controls

Add attach, launch-under-capture, existing-child discovery, new-child tracking,
start, stop, status, file location, file size, incomplete state, and retention
controls.

### OMEGA-SNIFF-05: Agent inspection

Add `inspect_network_capture`, a Network view, process and flow filters, and
sensitive-payload permission checks.

### OMEGA-SNIFF-06: Omega-launched proxy backend

Add stable flow identifiers and exact bidirectional chunk events to
`http_proxy`. Apply this optional backend to Omega-launched ACP agents.

### OMEGA-SNIFF-07: Optional TLS inspection

Add supported launch-only trust paths, TLS key-log import, and decoded protocol
records. Keep this work separate from raw flow capture.

### OMEGA-SNIFF-08: Linux and Windows application capture

Add the eBPF and Windows Filtering Platform backends. Keep the application
identity, attribution, capture file, and inspection contracts equal across
platforms.

## 13. Acceptance tests

The first release is complete when all of these tests pass:

1. A fixture application has no Omega integration and is open before capture.
   Attach mode captures new TCP and UDP traffic from that application.
2. Attach mode does not claim traffic from before its recorded attachment time.
3. Launch-under-capture records the first network flow of the fixture
   application.
4. A second application has the same display name and a different signer. Its
   traffic does not enter the capture.
5. A child that existed before attachment and a child that starts later both
   enter the verified target process tree.
6. Process ID reuse does not add an unrelated process to the capture.
7. The stream and datagram bytes match the bytes from the platform capture
   boundary.
8. TCP, UDP, QUIC, and DNS tests show their supported flows and attribution.
9. An unrelated system and application flow does not enter the capture file.
10. A capture component or writer failure marks the capture incomplete and does
    not stop the attached application.
11. A size-limit event stops capture without a silent byte gap.
12. An app restart recovers an unclean file and marks it incomplete.
13. `inspect_network_capture` can list targets, processes, destinations, and
    attribution. It can also read a bounded byte range.
14. The tool cannot read sensitive payload data without the required user
    grant.
15. No capture file enters Sync, telemetry, or the project worktree.
16. The capture indicator stays visible for the complete active interval.
17. A remote application gets a clear unsupported result. Omega does not
    create an empty file and call it complete.

## 14. Open decisions

These decisions need implementation research before the applicable delivery
item starts:

- the signed macOS Network Extension type and entitlement path
- the Windows Filtering Platform distribution and update path
- the Linux eBPF privilege and distribution path
- whether decrypted protocol records store full bodies or selected fields
- whether an inspected capture can use a separate encryption key
- whether a platform can also provide `os_packet_v1` and PCAPNG

The macOS capture component decision blocks `os_app_flow_v1`. The proxy work
does not remove that block because an independent application can ignore the
proxy.
