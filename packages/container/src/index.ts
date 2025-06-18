/**
 * Container orchestration and Firecracker VM management for OpenAgents
 * @since 0.0.0
 */

// Re-export all firecracker exports
export * as firecracker from "./firecracker/index.js"

// Core services (using stub implementations for MVP)
export * from "./firecracker/FirecrackerService.stub.js"
export * from "./firecracker/NetworkSetup.stub.js"

// Configuration schemas
export * from "./firecracker/MicroVMConfig.js"

// Errors
export * from "./firecracker/errors.js"

// Helper to create default Firecracker configuration
import { BootSource, Drive, FirecrackerConfig, MachineConfig } from "./firecracker/MicroVMConfig.js"

export const makeDefaultFirecrackerConfig = (options: {
  kernelPath: string
  rootfsPath: string
  vcpus?: number
  memoryMb?: number
  bootArgs?: string
}) => new FirecrackerConfig({
  boot_source: new BootSource({
    kernel_image_path: options.kernelPath,
    boot_args: options.bootArgs || "console=ttyS0 reboot=k panic=1 pci=off",
  }),
  drives: [
    new Drive({
      drive_id: "rootfs",
      path_on_host: options.rootfsPath,
      is_root_device: true,
      is_read_only: false,
    }),
  ],
  machine_config: new MachineConfig({
    vcpu_count: options.vcpus || 1,
    mem_size_mib: options.memoryMb || 256,
  }),
  network_interfaces: [],
})