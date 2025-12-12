# TestGen v2 + kv-store-grpc - TB2 FAIL

**Date:** 2024-12-11 19:39
**Status:** FAIL
**Model:** `claude-haiku-4-5-20251001`

## Summary

kv-store-grpc task failed due to infrastructure limitation - requires persistent container state.

| Metric | Value |
|--------|-------|
| Turns | 16 |
| Duration | 60.0s |
| Cost | $0.08 |
| TB2 Result | **FAIL (3/7 tests)** |

## Test Results

| Test | Result |
|------|--------|
| test_proto_file_creation | **PASS** |
| test_protobuf_generation | **PASS** |
| test_server_file_creation | **PASS** |
| test_grpc_tools_installation | FAIL |
| test_real_grpc_server_running | FAIL |
| test_grpc_protocol_handshake | FAIL |
| test_grpc_server_functionality | FAIL |

## What Agent Created

All required files were created correctly:
- `kv-store.proto` - Protocol buffer definition
- `kv_store_pb2.py` - Generated protobuf code
- `kv_store_pb2_grpc.py` - Generated gRPC code
- `server.py` - Server implementation

## Root Cause Analysis

**Infrastructure Limitation:** The task requires:
1. System-wide package installation that persists
2. Background server process that stays running

Our current setup:
1. Development container runs during agent execution
2. Agent installs packages via `docker exec ... pip install`
3. Agent starts server via `docker exec ... python server.py &`
4. Development container is STOPPED after agent finishes
5. Verification runs in FRESH container with workspace mounted

**Problem:** Fresh verification container doesn't have:
- Packages installed by agent (pip installs not persisted)
- Server process running (container was restarted)

## Task Requirements

From instruction:
> Install grpcio (1.73.0) and grpcio-tools (1.73.0) python packages system-wide.
> Run the server.py file and keep it running in the background.

These require **persistent container state**, not just file changes.

## Category Classification

This is **Category B** (infrastructure enhancement needed).

The task is designed for agents that run INSIDE the Docker container, not agents that exec INTO a container.

## Possible Fixes

### Option 1: Don't Stop Development Container
Keep the development container running for verification:
```bash
# Current (broken):
docker stop "${CONTAINER_NAME}"  # After agent
docker run ... "${DOCKER_IMAGE}" bash /tests/test.sh  # Fresh container

# Fixed:
# Don't stop container
docker exec "${CONTAINER_NAME}" bash /tests/test.sh  # Same container
```

### Option 2: Persist State in Workspace
Agent writes startup script to workspace:
```bash
# /app/startup.sh
pip install grpcio==1.73.0 grpcio-tools==1.73.0
python /app/server.py &
```
Verification runs startup.sh before tests.

### Option 3: Use Original TB2 Infrastructure
TB2's original design runs agent INSIDE container. Our tbench runs agent OUTSIDE.

## Pattern Update

| Task | Files Created | Persistent State | Result |
|------|--------------|------------------|--------|
| overfull-hbox | Files only | No | PASS |
| prove-plus-comm | Files only | No | FAIL (naming) |
| fix-git | Files only | No | FAIL (merge) |
| largest-eigenval | Files only | No | FAIL (perf) |
| kv-store-grpc | Files created | Required | FAIL |

**New Pattern:** Tasks requiring persistent container state fail with current infrastructure.

## Files

| File | Location |
|------|----------|
| ATIF Trajectory | `results/trajectories/kv-store-grpc/20251211-193815-81fb41a1/` |
| Workspace | `/tmp/tmp.NZhbP9EB6v/app` |

## Recommendation

1. **Short-term:** Skip tasks requiring persistent container state
2. **Long-term:** Modify tb2-run.sh to keep container running for verification

Tasks that require persistent state should be moved to Category B until infrastructure is enhanced.
