# Firecracker VM Architecture for OpenAgents Web

**Status:** Speculative / Future Architecture
**Last Updated:** December 2025
**Target:** AWS Production Deployment

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Why Firecracker?](#why-firecracker)
4. [Firecracker Integration](#firecracker-integration)
5. [OANIX in Firecracker](#oanix-in-firecracker)
6. [Communication Patterns](#communication-patterns)
7. [AWS Deployment](#aws-deployment)
8. [Security Model](#security-model)
9. [Web Frontend Architecture](#web-frontend-architecture)
10. [Snapshotting Strategy](#snapshotting-strategy)
11. [Implementation Roadmap](#implementation-roadmap)
12. [Trade-offs & Alternatives](#trade-offs--alternatives)
13. [Appendices](#appendices)

---

## Executive Summary

This document explores a **web-based OpenAgents platform** where users interact via a browser-based interface (Rust/WASM/WebGPU) while agents execute in isolated **Firecracker microVMs** on AWS infrastructure.

### The Vision

```
┌─────────────────────────────────────────────────────────────────────┐
│                         OpenAgents Web                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   Browser (WASM/WebGPU)     AWS API Layer      Firecracker VMs      │
│   ┌─────────────────┐      ┌────────────┐     ┌─────────────────┐   │
│   │                 │      │            │     │ ┌─────────────┐ │   │
│   │  GPUI Frontend  │◄────►│ Orchestrator│◄───►│ │ Agent VM 1  │ │   │
│   │  (MechaCoder)   │  WS  │   (Rust)   │vsock│ └─────────────┘ │   │
│   │                 │      │            │     │ ┌─────────────┐ │   │
│   └─────────────────┘      └────────────┘     │ │ Agent VM 2  │ │   │
│          ▲                       │            │ └─────────────┘ │   │
│          │                       │            │ ┌─────────────┐ │   │
│      User Input              Task Queue       │ │ Agent VM N  │ │   │
│      (keyboard,              (Nostr/NIP-90)   │ └─────────────┘ │   │
│       gestures)                               └─────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Why This Architecture?

| Concern | Docker Containers | Firecracker VMs |
|---------|------------------|-----------------|
| **Isolation** | Shared kernel (cgroups/namespaces) | Hardware virtualization (KVM) |
| **Boot time** | ~1-2 seconds | <125ms from snapshot |
| **Memory overhead** | ~50MB per container | <5MB per VM |
| **Multi-tenancy** | Risk of container escapes | Strong isolation guarantees |
| **Density** | ~100s per host | ~1000s per host |
| **AWS alignment** | Generic | Powers Lambda/Fargate |

### Target Use Case

- Users sign up at `openagents.com`
- Select an agent (Claude Code, Pi, custom agents)
- Provide a task via web interface
- Agent runs in dedicated Firecracker VM on AWS
- Stream results back to browser in real-time
- Pay per execution (via Lightning/Nostr)

---

## Architecture Overview

### Three-Tier Model

```
┌────────────────────────────────────────────────────────────────────────┐
│                           TIER 1: BROWSER                               │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                      GPUI (WASM + WebGPU)                         │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │  │
│  │  │ Task UI  │  │ Log View │  │ File Tree│  │ Terminal Emulator│  │  │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────────────┘  │  │
│  │                          │                                        │  │
│  │                    WebSocket Connection                           │  │
│  └──────────────────────────┼───────────────────────────────────────┘  │
└─────────────────────────────┼──────────────────────────────────────────┘
                              │
                              ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        TIER 2: AWS API LAYER                            │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                      Orchestrator Service                         │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐   │  │
│  │  │ WebSocket   │  │ Task Queue  │  │ Firecracker Manager    │   │  │
│  │  │ Gateway     │  │ (NIP-90)    │  │ (VM Pool + Snapshots)  │   │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────────────┘   │  │
│  │                                              │                    │  │
│  │                                         vsock API                 │  │
│  └──────────────────────────────────────────────┼───────────────────┘  │
└─────────────────────────────────────────────────┼──────────────────────┘
                                                  │
                                                  ▼
┌────────────────────────────────────────────────────────────────────────┐
│                      TIER 3: FIRECRACKER VMs                            │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                        EC2 Bare Metal Host                        │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐  │  │
│  │  │ microVM 1  │  │ microVM 2  │  │ microVM 3  │  │ microVM N  │  │  │
│  │  │ ┌────────┐ │  │ ┌────────┐ │  │ ┌────────┐ │  │ ┌────────┐ │  │  │
│  │  │ │ Agent  │ │  │ │ Agent  │ │  │ │ Agent  │ │  │ │ Agent  │ │  │  │
│  │  │ │ (OANIX)│ │  │ │ (OANIX)│ │  │ │ (OANIX)│ │  │ │ (OANIX)│ │  │  │
│  │  │ └────────┘ │  │ └────────┘ │  │ └────────┘ │  │ └────────┘ │  │  │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Location | Responsibility |
|-----------|----------|----------------|
| **GPUI Frontend** | Browser (WASM) | Task input, log streaming, file browsing |
| **WebSocket Gateway** | AWS (ECS/Lambda) | Connection management, authentication |
| **Task Queue** | AWS (with Nostr relay) | NIP-90 job requests, result events |
| **Firecracker Manager** | AWS (bare metal) | VM lifecycle, snapshot management |
| **Agent VMs** | Firecracker | Execute tasks in isolation |

### Data Flow: User Submits Task

```
1. User types task in browser
   │
2. GPUI serializes → WebSocket message
   │
3. Gateway authenticates → publishes NIP-90 job request
   │
4. Orchestrator picks job → allocates VM from pool
   │
5. VM restored from snapshot (<125ms)
   │
6. Workspace + task injected via MMDS/vsock
   │
7. Agent executes, streams logs via vsock
   │
8. Logs forwarded → WebSocket → Browser (real-time)
   │
9. Agent completes → results published as NIP-90 result event
   │
10. VM terminated or returned to pool
```

---

## Why Firecracker?

Firecracker is an open-source Virtual Machine Monitor (VMM) developed by AWS, written entirely in Rust. It powers AWS Lambda and Fargate, providing the isolation of VMs with the speed of containers.

### Key Capabilities

| Feature | Specification | Benefit for OpenAgents |
|---------|--------------|------------------------|
| **Boot time** | <125ms (from snapshot: <10ms) | Near-instant agent startup |
| **Memory overhead** | <5 MiB per VM | Run 1000+ agents per host |
| **CPU overhead** | >95% bare-metal performance | No performance tax |
| **Isolation** | KVM hardware virtualization | Tenant-safe multi-tenancy |
| **Snapshotting** | Full + diff snapshots | Instant workspace restore |
| **API** | REST over Unix socket | Programmatic VM control |
| **Language** | Rust | Ecosystem alignment |

### Firecracker vs Docker for Agent Execution

```
Docker Container                    Firecracker microVM
┌─────────────────────┐            ┌─────────────────────┐
│     Agent Code      │            │     Agent Code      │
├─────────────────────┤            ├─────────────────────┤
│   Container Runtime │            │    Guest Kernel     │
├─────────────────────┤            ├─────────────────────┤
│  Shared Host Kernel │            │   KVM Hypervisor    │
└─────────────────────┘            ├─────────────────────┤
                                   │    Host Kernel      │
                                   └─────────────────────┘

Isolation: Namespaces + cgroups    Isolation: Hardware + seccomp
Risk: Kernel exploits affect all   Risk: VM escapes extremely rare
```

### Security Comparison

| Attack Vector | Docker | Firecracker |
|---------------|--------|-------------|
| Kernel exploit | All containers affected | Only one VM affected |
| Container escape | Possible (CVE history) | Requires KVM escape (rare) |
| Resource exhaustion | cgroups (bypassable) | Rate limiting + jailer |
| Network snooping | Possible via host | No guest networking (vsock only) |

---

## Firecracker Integration

### OpenAgents Sandbox Abstraction

The existing `crates/sandbox/` provides a `ContainerBackend` trait that abstracts container runtimes. We add `FirecrackerBackend`:

```rust
// crates/sandbox/src/firecracker.rs

use crate::{ContainerBackend, ContainerConfig, ContainerResult};
use firecracker_client::{FirecrackerClient, VmConfig};

pub struct FirecrackerBackend {
    /// Pool of warm VMs ready for instant allocation
    vm_pool: VmPool,
    /// Path to base snapshot
    base_snapshot: PathBuf,
    /// Firecracker binary path
    firecracker_bin: PathBuf,
    /// Jailer binary path
    jailer_bin: PathBuf,
}

impl ContainerBackend for FirecrackerBackend {
    async fn create(&self, config: ContainerConfig) -> Result<ContainerId> {
        // 1. Allocate VM from pool or restore from snapshot
        let vm = self.vm_pool.acquire().await?;

        // 2. Inject workspace via MMDS
        vm.mmds_put("/task", &config.task_spec)?;
        vm.mmds_put("/workspace-url", &config.workspace_url)?;

        // 3. Start VM if not already running
        vm.start().await?;

        Ok(vm.id())
    }

    async fn exec(&self, id: ContainerId, cmd: &[&str]) -> Result<ExecResult> {
        let vm = self.vm_pool.get(id)?;

        // Execute via vsock connection
        let conn = vm.vsock_connect(AGENT_PORT).await?;
        conn.send_command(cmd).await?;

        // Stream output back
        let output = conn.recv_output().await?;
        Ok(output)
    }

    async fn logs(&self, id: ContainerId) -> Result<impl Stream<Item = LogLine>> {
        let vm = self.vm_pool.get(id)?;
        let conn = vm.vsock_connect(LOGS_PORT).await?;
        Ok(conn.stream_logs())
    }

    async fn destroy(&self, id: ContainerId) -> Result<()> {
        let vm = self.vm_pool.get(id)?;

        // Option 1: Return to pool (reset state)
        self.vm_pool.release(vm).await?;

        // Option 2: Destroy completely
        // vm.shutdown().await?;

        Ok(())
    }
}
```

### VM Lifecycle

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│   Snapshot  │─────►│  Warm Pool  │─────►│   Running   │
│   (on disk) │      │ (pre-booted)│      │ (executing) │
└─────────────┘      └─────────────┘      └─────────────┘
      ▲                     │                    │
      │                     │                    │
      │              ┌──────▼──────┐             │
      │              │   Recycle   │◄────────────┘
      │              │ (wipe state)│
      │              └─────────────┘
      │                     │
      └─────────────────────┘
         (periodic refresh)
```

### Firecracker API Integration

Firecracker exposes a REST API over Unix socket. Key endpoints:

```
PUT /machine-config          # Set vCPU count, memory
PUT /boot-source             # Set kernel, rootfs
PUT /drives/{id}             # Attach block devices
PUT /network-interfaces/{id} # Configure networking
PUT /vsock                   # Enable vsock device
PUT /mmds/config             # Configure metadata service
PUT /mmds                    # Set metadata content
PUT /actions                 # InstanceStart, SendCtrlAltDel
PUT /snapshot/create         # Snapshot VM
PUT /snapshot/load           # Restore from snapshot
```

Example: Start a VM

```bash
# Create socket
SOCKET=/tmp/firecracker.sock

# Configure VM
curl --unix-socket $SOCKET -X PUT \
  http://localhost/machine-config \
  -d '{"vcpu_count": 2, "mem_size_mib": 512}'

# Set kernel + rootfs
curl --unix-socket $SOCKET -X PUT \
  http://localhost/boot-source \
  -d '{
    "kernel_image_path": "/images/vmlinux",
    "boot_args": "console=ttyS0 reboot=k panic=1"
  }'

curl --unix-socket $SOCKET -X PUT \
  http://localhost/drives/rootfs \
  -d '{
    "drive_id": "rootfs",
    "path_on_host": "/images/agent-rootfs.ext4",
    "is_root_device": true,
    "is_read_only": false
  }'

# Enable vsock for host-guest communication
curl --unix-socket $SOCKET -X PUT \
  http://localhost/vsock \
  -d '{"guest_cid": 3, "uds_path": "/tmp/vsock.sock"}'

# Start VM
curl --unix-socket $SOCKET -X PUT \
  http://localhost/actions \
  -d '{"action_type": "InstanceStart"}'
```

---

## OANIX in Firecracker

OANIX is OpenAgents' Plan 9-inspired agent operating environment. It provides a filesystem abstraction for all capabilities. Inside a Firecracker VM, OANIX mounts look like:

### Namespace Mapping

```
Guest VM Filesystem:
/
├── task/                 # Task specification (via MMDS)
│   ├── spec.json        # Full task definition
│   └── meta.json        # Metadata, tags
│
├── workspace/           # Project files (via block device or vsock)
│   ├── src/
│   ├── Cargo.toml
│   └── ...
│
├── logs/                # Agent output (streamed via vsock)
│   ├── stdout.log
│   ├── stderr.log
│   └── events/          # JSONL event stream
│
├── cap/                 # Capabilities (proxied via vsock)
│   ├── http/           # HTTP requests (proxied to host)
│   │   ├── outbox/     # Queue request
│   │   └── inbox/      # Read response
│   │
│   ├── nostr/          # Nostr events (via host relay)
│   │   ├── submit      # Publish event
│   │   └── events      # Subscription stream
│   │
│   └── host/           # Host communication (vsock)
│       ├── control     # VM lifecycle commands
│       └── files       # Host file access (limited)
│
└── tmp/                 # Scratch space (tmpfs)
```

### vsock-Based Capability Bridge

Instead of giving agents direct network access, all capabilities are proxied through vsock:

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Guest VM                                   │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                       OANIX Agent                            │    │
│  │                            │                                 │    │
│  │   read("/cap/http/inbox")  │  write("/cap/http/outbox")     │    │
│  │              │              │              │                 │    │
│  └──────────────┼──────────────┼──────────────┼─────────────────┘    │
│                 │              │              │                      │
│  ┌──────────────▼──────────────▼──────────────▼─────────────────┐    │
│  │              CapabilityFs (vsock client)                      │    │
│  └───────────────────────────┬──────────────────────────────────┘    │
└──────────────────────────────┼──────────────────────────────────────┘
                               │ vsock (CID 3, port 5000)
┌──────────────────────────────┼──────────────────────────────────────┐
│                              ▼                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │              CapabilityProxy (vsock server)                    │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │  │
│  │  │ HttpExecutor│  │NostrConnector│  │ WsConnector │            │  │
│  │  │  (reqwest)  │  │  (relay)     │  │ (tungstenite)│           │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘            │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              Host                                    │
└─────────────────────────────────────────────────────────────────────┘
```

### Agent Execution Flow

```rust
// Inside guest VM - agent startup

fn main() {
    // 1. Read task from MMDS-populated file
    let task: TaskSpec = serde_json::from_reader(
        File::open("/task/spec.json")?
    )?;

    // 2. Initialize OANIX namespace
    let namespace = Namespace::builder()
        .mount("/task", TaskFs::from_file("/task"))
        .mount("/workspace", WorkspaceFs::new("/workspace"))
        .mount("/logs", LogsFs::new("/logs"))
        .mount("/cap/http", VsockHttpFs::new(CID_HOST, HTTP_PORT))
        .mount("/cap/nostr", VsockNostrFs::new(CID_HOST, NOSTR_PORT))
        .mount("/tmp", MemFs::new())
        .build();

    // 3. Run agent
    let agent = PiAgent::new(namespace);
    agent.execute(&task).await?;

    // 4. Signal completion via vsock
    VsockControl::new(CID_HOST, CONTROL_PORT)
        .signal_complete()?;
}
```

---

## Communication Patterns

### vsock vs Network

Firecracker VMs can have traditional virtio-net networking, but for security we use **vsock** (virtual sockets) exclusively:

| Feature | Network (virtio-net) | vsock |
|---------|---------------------|-------|
| Protocol | TCP/UDP over IP | Stream/datagram over vsock |
| Addressing | IP:Port | CID:Port |
| Host access | Via NAT/bridge | Direct socket |
| Security | Firewall rules needed | No network exposure |
| Latency | Higher (network stack) | Lower (direct) |

### vsock Port Allocation

```
CID 2  = Host (reserved)
CID 3+ = Guest VMs (assigned by Firecracker)

Port Layout (per VM):
5000 = Control channel (lifecycle commands)
5001 = HTTP capability proxy
5002 = WebSocket capability proxy
5003 = Nostr capability proxy
5004 = Log streaming
5005 = File transfer
```

### Streaming Logs

Logs stream from VM to browser via vsock → WebSocket pipeline:

```
Guest Agent          Host Proxy          WebSocket Gateway        Browser
    │                    │                      │                    │
    │ write("/logs/      │                      │                    │
    │   stdout.log")     │                      │                    │
    │───────────────────►│                      │                    │
    │                    │ vsock frame          │                    │
    │                    │─────────────────────►│                    │
    │                    │                      │ ws.send()          │
    │                    │                      │───────────────────►│
    │                    │                      │                    │ display
```

### HTTP API Proxying

Agent HTTP requests are proxied through the host:

```rust
// Guest: VsockHttpFs
async fn http_request(req: HttpRequest) -> HttpResponse {
    // Serialize request
    let frame = VsockFrame::HttpRequest(req);

    // Send via vsock
    self.conn.send(frame).await?;

    // Wait for response
    let resp = self.conn.recv().await?;
    resp.into_http_response()
}

// Host: HttpProxy
async fn handle_request(frame: VsockFrame) -> VsockFrame {
    let req = frame.into_http_request();

    // Apply rate limiting
    self.rate_limiter.check(&req)?;

    // Execute via reqwest
    let resp = self.client.execute(req).await?;

    VsockFrame::HttpResponse(resp)
}
```

---

## AWS Deployment

### Instance Selection

Firecracker requires KVM access, available on:

| Instance | vCPUs | Memory | Network | VMs @ 512MB | Cost/hr |
|----------|-------|--------|---------|-------------|---------|
| `i3.metal` | 72 | 512 GB | 25 Gbps | ~1000 | ~$5.00 |
| `m5.metal` | 96 | 384 GB | 25 Gbps | ~750 | ~$4.50 |
| `m5zn.metal` | 48 | 192 GB | 100 Gbps | ~375 | ~$3.50 |
| `c5.metal` | 96 | 192 GB | 25 Gbps | ~375 | ~$4.00 |

**Recommended:** Start with `m5.metal` for balance of CPU/memory.

### Multi-VM Density

With 512 MB per VM and 5 MiB Firecracker overhead:

```
m5.metal: 384 GB RAM
  - Reserve 16 GB for host
  - Available: 368 GB
  - VMs @ 512 MB: 736 concurrent VMs
  - VMs @ 256 MB: 1472 concurrent VMs
```

### Architecture on AWS

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              AWS Region                                  │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                         VPC (10.0.0.0/16)                          │ │
│  │                                                                     │ │
│  │  ┌─────────────────────────────────────────────────────────────┐   │ │
│  │  │                    Public Subnet                             │   │ │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │   │ │
│  │  │  │     ALB     │  │  API GW     │  │   CloudFront (CDN)  │  │   │ │
│  │  │  │ (WebSocket) │  │  (REST)     │  │   (Static Assets)   │  │   │ │
│  │  │  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │   │ │
│  │  └─────────┼────────────────┼──────────────────────────────────┘   │ │
│  │            │                │                                       │ │
│  │  ┌─────────▼────────────────▼──────────────────────────────────┐   │ │
│  │  │                    Private Subnet                            │   │ │
│  │  │  ┌─────────────────────────────────────────────────────────┐ │   │ │
│  │  │  │              Orchestrator Service (ECS Fargate)         │ │   │ │
│  │  │  │  ┌───────────┐  ┌───────────┐  ┌───────────────────┐   │ │   │ │
│  │  │  │  │ WS Handler│  │ Task Queue│  │ Firecracker Mgr   │   │ │   │ │
│  │  │  │  └───────────┘  └───────────┘  └─────────┬─────────┘   │ │   │ │
│  │  │  └──────────────────────────────────────────┼─────────────┘ │   │ │
│  │  │                                             │                │   │ │
│  │  │  ┌──────────────────────────────────────────▼─────────────┐ │   │ │
│  │  │  │           Firecracker Fleet (Bare Metal EC2)           │ │   │ │
│  │  │  │  ┌────────────┐  ┌────────────┐  ┌────────────┐       │ │   │ │
│  │  │  │  │ m5.metal-1 │  │ m5.metal-2 │  │ m5.metal-N │       │ │   │ │
│  │  │  │  │ ~700 VMs   │  │ ~700 VMs   │  │ ~700 VMs   │       │ │   │ │
│  │  │  │  └────────────┘  └────────────┘  └────────────┘       │ │   │ │
│  │  │  └────────────────────────────────────────────────────────┘ │   │ │
│  │  │                                                              │   │ │
│  │  │  ┌────────────────────────────────────────────────────────┐ │   │ │
│  │  │  │                    Data Layer                          │ │   │ │
│  │  │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌───────────┐ │ │   │ │
│  │  │  │  │   S3    │  │   EBS   │  │ DynamoDB│  │ ElastiCache│ │ │   │ │
│  │  │  │  │(snapshots)│ │(rootfs) │  │ (state) │  │  (cache)  │ │ │   │ │
│  │  │  │  └─────────┘  └─────────┘  └─────────┘  └───────────┘ │ │   │ │
│  │  │  └────────────────────────────────────────────────────────┘ │   │ │
│  │  └──────────────────────────────────────────────────────────────┘   │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

### Autoscaling Strategy

```rust
// Autoscaling logic
struct AutoScaler {
    target_vm_utilization: f64,  // e.g., 0.7 (70%)
    min_hosts: usize,
    max_hosts: usize,
}

impl AutoScaler {
    fn desired_hosts(&self, metrics: &Metrics) -> usize {
        let total_capacity = metrics.hosts * VMS_PER_HOST;
        let current_utilization = metrics.active_vms as f64 / total_capacity as f64;

        if current_utilization > self.target_vm_utilization {
            // Scale up
            (metrics.hosts + 1).min(self.max_hosts)
        } else if current_utilization < self.target_vm_utilization * 0.5 {
            // Scale down (with cooldown)
            (metrics.hosts - 1).max(self.min_hosts)
        } else {
            metrics.hosts
        }
    }
}
```

### Cost Analysis

Assumptions:
- 1000 tasks/day
- Average task duration: 5 minutes
- VM size: 512 MB RAM, 1 vCPU

```
Daily VM-minutes: 1000 × 5 = 5000 VM-minutes
Peak concurrent VMs: ~50 (assuming even distribution)

Infrastructure:
- 1× m5.metal ($4.50/hr × 24) = $108/day
- Handles: 700+ concurrent VMs (14× headroom)

Per-task cost:
- $108 / 1000 tasks = $0.108 per task

At scale (10,000 tasks/day):
- 2× m5.metal = $216/day
- Per-task: $0.022

Break-even vs Lambda:
- Lambda @ 512MB, 5min: ~$0.05/invocation
- Firecracker: ~$0.02/invocation (at scale)
```

---

## Security Model

### Defense in Depth

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Layer 1: Network Isolation                        │
│  - Private subnet, no public IPs on Firecracker hosts               │
│  - VMs have no network interfaces (vsock only)                      │
│  - All external traffic proxied through host                        │
└─────────────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────▼───────────────────────────────────────┐
│                    Layer 2: Jailer Sandbox                           │
│  - chroot jail per VM                                                │
│  - New PID namespace                                                 │
│  - New mount namespace                                               │
│  - cgroups v2 resource limits                                        │
│  - Dropped privileges (non-root)                                     │
└─────────────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────▼───────────────────────────────────────┐
│                    Layer 3: Seccomp Filters                          │
│  - Firecracker uses thread-specific seccomp filters                 │
│  - Only ~35 syscalls allowed for VMM                                │
│  - Separate filters for API, VMM, vCPU threads                      │
└─────────────────────────────────────────────────────────────────────┘
                               │
┌─────────────────────────────▼───────────────────────────────────────┐
│                    Layer 4: KVM Virtualization                       │
│  - Hardware-enforced memory isolation                                │
│  - Guest cannot access host memory                                   │
│  - IOMMU protection for device passthrough                          │
└─────────────────────────────────────────────────────────────────────┘
```

### Jailer Configuration

```bash
# Production jailer invocation
jailer \
  --id vm-${UUID} \
  --exec-file /usr/bin/firecracker \
  --uid 1000 \
  --gid 1000 \
  --chroot-base-dir /srv/jailer \
  --cgroup-version 2 \
  --cgroup cpuset.cpus=0-1 \
  --cgroup memory.max=536870912 \
  --netns /var/run/netns/fc-${UUID}
```

### Credential Injection

API keys and secrets are injected via MMDS (Microvm Metadata Service):

```rust
// Host: inject credentials before VM start
async fn inject_credentials(vm: &Vm, user: &User) -> Result<()> {
    // Generate short-lived tokens
    let api_token = generate_scoped_token(user, Duration::from_secs(3600))?;

    // Inject via MMDS (accessible at 169.254.169.254)
    vm.mmds_put("/secrets/api-token", &api_token).await?;

    // Never inject long-lived credentials
    // vm.mmds_put("/secrets/api-key", &user.api_key)?;  // BAD!

    Ok(())
}

// Guest: read credentials
fn get_api_token() -> String {
    let resp = reqwest::get("http://169.254.169.254/secrets/api-token")
        .await?;
    resp.text().await?
}
```

### Tenant Isolation Guarantees

| Guarantee | Mechanism |
|-----------|-----------|
| Memory isolation | KVM EPT/NPT, separate address spaces |
| CPU isolation | Separate vCPU threads, cgroup quotas |
| Disk isolation | Separate block devices per VM |
| Network isolation | No guest networking (vsock only) |
| Metadata isolation | Separate MMDS instance per VM |
| Process isolation | Separate PID namespaces |
| Filesystem isolation | chroot jails, separate rootfs |

---

## Web Frontend Architecture

### GPUI to WASM

The MechaCoder frontend (GPUI-based) compiles to WASM for browser execution:

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Browser                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    GPUI (wasm32-unknown-unknown)               │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐   │  │
│  │  │ TaskPanel   │  │ LogPanel    │  │ WorkspacePanel      │   │  │
│  │  │ (input)     │  │ (streaming) │  │ (file tree, editor) │   │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘   │  │
│  │                           │                                   │  │
│  │                    ┌──────▼──────┐                            │  │
│  │                    │ WebGPU/WebGL│                            │  │
│  │                    │ (rendering) │                            │  │
│  │                    └─────────────┘                            │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                       WebSocket                                      │
│                              │                                       │
└──────────────────────────────┼──────────────────────────────────────┘
                               │
                               ▼
                        AWS API Layer
```

### WebGPU Rendering

GPUI's Blade renderer has experimental WebGL/GLES support. For production:

```rust
// crates/gpui/src/platform/web/renderer.rs

#[cfg(target_arch = "wasm32")]
use wgpu::{Device, Queue, Surface};

pub struct WebRenderer {
    device: Device,
    queue: Queue,
    surface: Surface,
}

impl WebRenderer {
    pub async fn new(canvas: HtmlCanvasElement) -> Self {
        let instance = wgpu::Instance::new(wgpu::Backends::BROWSER_WEBGPU);
        let surface = instance.create_surface_from_canvas(&canvas)?;

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }).await?;

        let (device, queue) = adapter.request_device(&Default::default(), None).await?;

        Self { device, queue, surface }
    }

    pub fn render(&mut self, scene: &Scene) {
        // Render GPUI scene graph via wgpu
    }
}
```

### WebSocket Protocol

```rust
// Client ↔ Server message types

#[derive(Serialize, Deserialize)]
enum ClientMessage {
    // Task submission
    SubmitTask { task: TaskSpec },
    CancelTask { task_id: Uuid },

    // File operations
    ReadFile { path: String },
    WriteFile { path: String, content: Vec<u8> },

    // Terminal
    TerminalInput { data: Vec<u8> },
    ResizeTerminal { rows: u16, cols: u16 },
}

#[derive(Serialize, Deserialize)]
enum ServerMessage {
    // Task lifecycle
    TaskQueued { task_id: Uuid },
    TaskStarted { task_id: Uuid, vm_id: Uuid },
    TaskCompleted { task_id: Uuid, result: TaskResult },
    TaskFailed { task_id: Uuid, error: String },

    // Streaming
    LogChunk { task_id: Uuid, data: Vec<u8> },
    FileContent { path: String, content: Vec<u8> },

    // Terminal
    TerminalOutput { data: Vec<u8> },
}
```

---

## Snapshotting Strategy

### Snapshot Types

```
┌─────────────────────────────────────────────────────────────────────┐
│                       Base Snapshot                                  │
│  - Full VM state: memory + guest kernel state                       │
│  - Created once per agent type                                      │
│  - Stored on S3, cached on local NVMe                               │
│  - Size: ~200-500 MB (depending on memory)                          │
└─────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       Diff Snapshot                                  │
│  - Only changed pages since base                                    │
│  - Per-task workspace state                                         │
│  - Fast to create and restore                                       │
│  - Size: ~10-50 MB (typical)                                        │
└─────────────────────────────────────────────────────────────────────┘
```

### Warm Pool Management

```rust
struct WarmPool {
    /// Pre-booted VMs ready for instant allocation
    ready: VecDeque<WarmVm>,
    /// Target pool size
    target_size: usize,
    /// Background task refilling pool
    refill_task: JoinHandle<()>,
}

impl WarmPool {
    async fn acquire(&mut self) -> Result<WarmVm> {
        // Get from pool (instant) or restore from snapshot (<125ms)
        match self.ready.pop_front() {
            Some(vm) => Ok(vm),
            None => self.restore_from_snapshot().await,
        }
    }

    async fn release(&mut self, mut vm: WarmVm) {
        // Wipe VM state for reuse
        vm.reset_to_base_snapshot().await?;
        self.ready.push_back(vm);
    }

    async fn maintain_pool_size(&self) {
        loop {
            while self.ready.len() < self.target_size {
                if let Ok(vm) = self.restore_from_snapshot().await {
                    self.ready.push_back(vm);
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
```

### Memory Deduplication (KSM)

For hosts running many similar VMs, enable Kernel Same-page Merging:

```bash
# Enable KSM on host
echo 1 > /sys/kernel/mm/ksm/run
echo 1000 > /sys/kernel/mm/ksm/sleep_millisecs

# Check savings
cat /sys/kernel/mm/ksm/pages_sharing
# Typical: 30-50% memory reduction for similar VMs
```

---

## Implementation Roadmap

### Phase 1: FirecrackerBackend PoC

**Goal:** Run a single agent in Firecracker on local machine.

**Tasks:**
- [ ] Create minimal rootfs with agent binary
- [ ] Implement `FirecrackerBackend` for `crates/sandbox/`
- [ ] Basic vsock communication (logs only)
- [ ] Manual VM lifecycle (no pooling)

**Deliverable:** `cargo run --example firecracker-agent`

### Phase 2: vsock Communication

**Goal:** Full capability proxying via vsock.

**Tasks:**
- [ ] Implement vsock-based CapabilityFs in guest
- [ ] Host-side capability proxy (HTTP, Nostr)
- [ ] Streaming log forwarding
- [ ] File transfer protocol

**Deliverable:** Agent completes HTTP-based task in VM

### Phase 3: Web Frontend (WASM)

**Goal:** Browser-based task submission and monitoring.

**Tasks:**
- [ ] GPUI/wgpu WASM compilation
- [ ] WebSocket client for orchestrator
- [ ] Log streaming UI
- [ ] Task submission form

**Deliverable:** Submit task from browser, see results

### Phase 4: AWS Production Deploy

**Goal:** Multi-tenant production deployment on AWS.

**Tasks:**
- [ ] Terraform/Pulumi infrastructure
- [ ] Bare metal fleet management
- [ ] Jailer security configuration
- [ ] ALB + WebSocket routing
- [ ] S3 snapshot storage
- [ ] Monitoring (CloudWatch, Prometheus)

**Deliverable:** Public beta at openagents.com

### Phase 5: Snapshotting + Scaling

**Goal:** Sub-second cold starts, autoscaling.

**Tasks:**
- [ ] Base snapshot generation pipeline
- [ ] Warm pool management
- [ ] Autoscaling based on queue depth
- [ ] KSM for memory efficiency
- [ ] Multi-region deployment

**Deliverable:** <1s task start time, 10,000+ tasks/day capacity

---

## Trade-offs & Alternatives

### Firecracker vs Docker Containers

| Aspect | Docker | Firecracker |
|--------|--------|-------------|
| Isolation | Process-level (namespaces) | Hardware-level (KVM) |
| Boot time | ~1-2s | <125ms (from snapshot) |
| Overhead | ~50MB | <5MB |
| Maturity | Very mature | Production-ready (AWS) |
| Ecosystem | Rich (Docker Hub) | Limited (custom rootfs) |
| Debugging | Easy (docker exec) | Harder (vsock or console) |

**Choose Docker when:** Rapid prototyping, trusted workloads, existing container infrastructure.

**Choose Firecracker when:** Multi-tenant production, untrusted code, AWS deployment.

### Firecracker vs gVisor

gVisor is Google's application kernel that intercepts syscalls:

| Aspect | gVisor | Firecracker |
|--------|--------|-------------|
| Isolation | User-space kernel | Hardware VM |
| Compatibility | ~80% Linux syscalls | Full Linux kernel |
| Performance | Variable (syscall overhead) | Near-native |
| Security | Good (reduced attack surface) | Strong (KVM) |

**Choose gVisor when:** Need container compatibility, syscall filtering is sufficient.

**Choose Firecracker when:** Need full Linux compatibility, stronger isolation guarantees.

### Firecracker vs Kata Containers

Kata Containers uses Firecracker or QEMU as a runtime for Kubernetes:

| Aspect | Kata Containers | Raw Firecracker |
|--------|-----------------|-----------------|
| Integration | Kubernetes-native | Custom orchestration |
| Complexity | Higher (OCI runtime) | Lower (direct API) |
| Features | Pod networking, volumes | Basic VM primitives |
| Control | Less (runtime decisions) | Full (custom logic) |

**Choose Kata when:** Using Kubernetes, want drop-in container replacement.

**Choose Raw Firecracker when:** Building custom platform, need full control.

### Open Questions

1. **Workspace transfer:** Should we use block devices (virtio-blk) or vsock-based file transfer for workspaces? Block is faster for large repos, vsock is simpler.

2. **Agent binary distribution:** Pre-bake into rootfs, or transfer at runtime? Pre-baked is faster, runtime is more flexible.

3. **Cost model:** Per-minute VM billing or per-task? Per-task is simpler for users, per-minute is more accurate.

4. **Multi-region:** Active-active or primary-failover? Active-active reduces latency but increases complexity.

5. **Hibernation:** Should long-running agents hibernate to save resources? Complicates state management but could reduce costs.

---

## Appendices

### Appendix A: Firecracker API Reference

Key API endpoints for VM lifecycle:

```yaml
# Machine configuration
PUT /machine-config
{
  "vcpu_count": 2,
  "mem_size_mib": 512,
  "smt": false
}

# Boot source
PUT /boot-source
{
  "kernel_image_path": "/images/vmlinux",
  "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"
}

# Root filesystem
PUT /drives/rootfs
{
  "drive_id": "rootfs",
  "path_on_host": "/images/agent.ext4",
  "is_root_device": true,
  "is_read_only": false
}

# vsock device
PUT /vsock
{
  "guest_cid": 3,
  "uds_path": "/tmp/vsock.sock"
}

# Metadata service
PUT /mmds/config
{
  "network_interfaces": ["eth0"],
  "ipv4_address": "169.254.169.254"
}

PUT /mmds
{
  "task": { "id": "...", "spec": "..." },
  "secrets": { "api_token": "..." }
}

# Start VM
PUT /actions
{
  "action_type": "InstanceStart"
}

# Create snapshot
PUT /snapshot/create
{
  "snapshot_type": "Full",
  "snapshot_path": "/snapshots/vm.snap",
  "mem_file_path": "/snapshots/vm.mem"
}

# Load snapshot
PUT /snapshot/load
{
  "snapshot_path": "/snapshots/vm.snap",
  "mem_backend": {
    "backend_type": "File",
    "backend_path": "/snapshots/vm.mem"
  },
  "enable_diff_snapshots": true
}
```

### Appendix B: VM Rootfs Creation

Create a minimal rootfs for agents:

```bash
#!/bin/bash
# create-rootfs.sh

# Start with Alpine base (small)
docker run --name rootfs-builder -it alpine:3.18 sh -c "
  # Install runtime dependencies
  apk add --no-cache \
    bash \
    curl \
    git \
    openssh-client \
    ca-certificates

  # Create agent user
  adduser -D -s /bin/bash agent

  # Create standard directories
  mkdir -p /workspace /logs /task /tmp
  chown agent:agent /workspace /logs /task /tmp
"

# Export filesystem
docker export rootfs-builder > rootfs.tar

# Create ext4 image
truncate -s 1G agent.ext4
mkfs.ext4 agent.ext4
mkdir /tmp/rootfs-mount
mount agent.ext4 /tmp/rootfs-mount
tar -xf rootfs.tar -C /tmp/rootfs-mount

# Copy agent binary
cp target/release/openagents-agent /tmp/rootfs-mount/usr/local/bin/

# Create init script
cat > /tmp/rootfs-mount/init << 'EOF'
#!/bin/bash
# Fetch task from MMDS
curl -s http://169.254.169.254/task > /task/spec.json

# Run agent
exec /usr/local/bin/openagents-agent
EOF
chmod +x /tmp/rootfs-mount/init

# Unmount
umount /tmp/rootfs-mount

# Upload to S3
aws s3 cp agent.ext4 s3://openagents-images/agent.ext4
```

### Appendix C: AWS Instance Type Selection

Detailed comparison for Firecracker workloads:

| Instance | vCPU | Memory | Storage | Network | Price | VMs @ 512MB |
|----------|------|--------|---------|---------|-------|-------------|
| `m5.metal` | 96 | 384 GB | EBS | 25 Gbps | $4.608/hr | 700 |
| `m5d.metal` | 96 | 384 GB | 4×900GB NVMe | 25 Gbps | $5.424/hr | 700 |
| `m5zn.metal` | 48 | 192 GB | EBS | 100 Gbps | $3.964/hr | 350 |
| `c5.metal` | 96 | 192 GB | EBS | 25 Gbps | $4.080/hr | 350 |
| `c5d.metal` | 96 | 192 GB | 4×900GB NVMe | 25 Gbps | $4.608/hr | 350 |
| `i3.metal` | 72 | 512 GB | 8×1.9TB NVMe | 25 Gbps | $4.992/hr | 950 |
| `r5.metal` | 96 | 768 GB | EBS | 25 Gbps | $6.048/hr | 1400 |

**Recommendations:**
- **Cost-optimized:** `m5.metal` - good balance, no local storage (use EBS)
- **Storage-heavy:** `i3.metal` - best for snapshot-heavy workloads
- **Memory-heavy:** `r5.metal` - highest VM density
- **Network-heavy:** `m5zn.metal` - 100 Gbps for high-bandwidth agents

### Appendix D: Cost Projections

Monthly cost at various scales:

| Tasks/Day | Peak VMs | Hosts Needed | Monthly Cost | Per-Task |
|-----------|----------|--------------|--------------|----------|
| 100 | 5 | 1 (m5.large) | $67 | $0.022 |
| 1,000 | 50 | 1 (m5.metal) | $3,318 | $0.108 |
| 10,000 | 500 | 2 (m5.metal) | $6,636 | $0.022 |
| 100,000 | 5,000 | 10 (m5.metal) | $33,178 | $0.011 |
| 1,000,000 | 50,000 | 80 (m5.metal) | $265,420 | $0.009 |

Notes:
- Assumes 5-minute average task duration
- Does not include data transfer, S3, monitoring costs
- Spot instances could reduce costs by 60-70%
- Reserved instances reduce costs by 30-40%

---

**Last updated:** December 12, 2025
**Author:** OpenAgents Team
**License:** Apache 2.0
