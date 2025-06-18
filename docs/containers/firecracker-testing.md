# Firecracker Container Testing Guide

This guide explains how to test the Firecracker container package MVP implementation.

## Overview

The `@openagentsinc/container` package provides Effect-based services for managing Firecracker microVMs. This MVP includes:

- FirecrackerService for VM lifecycle management
- NetworkSetupService for TAP interface configuration
- Integration with SDK's Compute namespace
- CLI commands for container operations

## Prerequisites

### 1. Install Firecracker

Follow the instructions in `docs/containers/firecracker-setup.md` or:

```bash
# Download Firecracker binary
ARCH=$(uname -m)
wget https://github.com/firecracker-microvm/firecracker/releases/download/v1.5.0/firecracker-v1.5.0-${ARCH}.tgz
tar -xzf firecracker-v1.5.0-${ARCH}.tgz
sudo mv release-v1.5.0-${ARCH}/firecracker-v1.5.0-${ARCH} /usr/local/bin/firecracker
sudo chmod +x /usr/local/bin/firecracker
```

### 2. Download Test Kernel and Root Filesystem

```bash
# Create directory for assets
mkdir -p ~/firecracker-assets

# Download minimal kernel
wget -O ~/firecracker-assets/vmlinux.bin https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/kernels/vmlinux.bin

# Download Alpine Linux rootfs
wget -O ~/firecracker-assets/alpine.ext4 https://s3.amazonaws.com/spec.ccfc.min/ci-artifacts/disks/alpine/alpine-rootfs.ext4
```

### 3. Verify KVM Support

```bash
# Check KVM is available
ls /dev/kvm
# Should output: /dev/kvm

# Check virtualization enabled
grep -E 'vmx|svm' /proc/cpuinfo
# Should show virtualization flags
```

## Testing the Implementation

### 1. Build the Packages

```bash
# Install dependencies
pnpm install

# Build all packages
pnpm build

# Run tests
pnpm test
```

### 2. Test via CLI (Stub Implementation)

The current implementation provides stub methods that simulate container operations:

```bash
# Test the container commands
pnpm --filter=@openagentsinc/cli run cli container test

# Deploy an agent (stub)
pnpm --filter=@openagentsinc/cli run cli container deploy agent123

# Check status (stub)
pnpm --filter=@openagentsinc/cli run cli container status deployment_agent123_1234567890

# Hibernate container (stub)
pnpm --filter=@openagentsinc/cli run cli container hibernate deployment_agent123_1234567890

# Wake container (stub)
pnpm --filter=@openagentsinc/cli run cli container wake deployment_agent123_1234567890
```

### 3. Test with Real Firecracker (Requires Root)

To test with actual Firecracker VMs, you need root permissions:

```bash
# Run as root or with sudo
sudo pnpm --filter=@openagentsinc/cli run cli container deploy agent123 \
  --kernel ~/firecracker-assets/vmlinux.bin \
  --rootfs ~/firecracker-assets/alpine.ext4 \
  --vcpus 1 \
  --memory 256
```

### 4. Test the SDK Integration

Create a test script to verify SDK integration:

```typescript
// test-sdk.ts
import * as SDK from "@openagentsinc/sdk"

// Create an agent
const agent = SDK.Agent.create({ name: "TestAgent" })
console.log("Created agent:", agent.id)

// Deploy to container (stub)
const deployment = SDK.Compute.deployToContainer(agent, {
  vcpus: 2,
  memoryMb: 512,
  networkEnabled: true
})
console.log("Deployment:", deployment)

// Check status (stub)
const status = SDK.Compute.getContainerStatus(deployment.id)
console.log("Status:", status)

// Hibernate (stub)
const hibernateResult = SDK.Compute.hibernateContainer(deployment.id)
console.log("Hibernate:", hibernateResult)

// Wake (stub)
const wakeResult = SDK.Compute.wakeContainer(deployment.id)
console.log("Wake:", wakeResult)
```

Run with:
```bash
tsx test-sdk.ts
```

## Architecture Details

### FirecrackerService

The Effect service manages VM lifecycle:
- Creates VM configuration files
- Starts Firecracker process
- Tracks VM state in memory
- Handles cleanup on shutdown

### NetworkSetupService

Manages network interfaces:
- Creates TAP devices (requires CAP_NET_ADMIN)
- Generates MAC addresses
- Configures bridge networking

### Integration Points

1. **SDK Compute Namespace**: Extended with container deployment methods
2. **CLI Commands**: New `container` subcommand with deploy, status, hibernate, wake operations
3. **Effect Services**: Proper dependency injection and error handling

## Next Steps for Full Implementation

1. **API Socket Communication**: Implement Firecracker API client for proper VM control
2. **CRIU Integration**: Add checkpoint/restore for hibernation
3. **Resource Monitoring**: Track actual CPU/memory usage
4. **Persistent State**: Store VM state in database instead of memory
5. **Multi-VM Orchestration**: Handle thousands of VMs efficiently
6. **Storage Deduplication**: Implement overlay filesystems
7. **Network Isolation**: Proper VLAN/namespace configuration

## Troubleshooting

### Permission Errors
```bash
# Add user to kvm group
sudo usermod -aG kvm $USER
newgrp kvm

# Or run with sudo
sudo pnpm --filter=@openagentsinc/cli run cli container deploy ...
```

### Firecracker Not Found
```bash
# Check binary location
which firecracker

# Update PATH if needed
export PATH=$PATH:/usr/local/bin
```

### KVM Not Available
- Ensure virtualization is enabled in BIOS
- Check kernel modules: `lsmod | grep kvm`
- Load modules if needed: `sudo modprobe kvm_intel` or `sudo modprobe kvm_amd`

## Development Workflow

1. Make changes to container package
2. Run `pnpm build` to compile
3. Test with CLI commands
4. Run unit tests: `pnpm --filter=@openagentsinc/container test`
5. For integration testing, use real Firecracker with sudo

## Security Considerations

- Firecracker requires KVM access (root or specific capabilities)
- TAP device creation needs CAP_NET_ADMIN
- Production deployments should use proper isolation and least privilege
- Consider using systemd service with appropriate permissions

## Performance Testing

Once full implementation is complete:

1. Test cold start times (target: <125ms)
2. Measure memory overhead (target: 5MB per VM)
3. Test density (target: 10,000 VMs per server)
4. Benchmark network throughput
5. Test hibernation/wake times with CRIU