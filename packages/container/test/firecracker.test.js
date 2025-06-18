import { describe, expect, it } from "@effect/vitest";
import { Effect, Layer } from "effect";
import { FirecrackerService, FirecrackerServiceLive, NetworkSetupService, NetworkSetupServiceLive, CreateVMRequest, StopVMRequest, makeDefaultFirecrackerConfig, } from "../src/index.js";
describe("FirecrackerService", () => {
    describe("Schema Validation", () => {
        it("validates FirecrackerConfig", () => {
            const config = makeDefaultFirecrackerConfig({
                kernelPath: "/path/to/kernel",
                rootfsPath: "/path/to/rootfs",
                vcpus: 2,
                memoryMb: 512,
            });
            expect(config.boot_source.kernel_image_path).toBe("/path/to/kernel");
            expect(config.boot_source.boot_args).toBe("console=ttyS0 reboot=k panic=1 pci=off");
            expect(config.drives).toHaveLength(1);
            expect(config.drives[0].drive_id).toBe("rootfs");
            expect(config.drives[0].path_on_host).toBe("/path/to/rootfs");
            expect(config.drives[0].is_root_device).toBe(true);
            expect(config.machine_config.vcpu_count).toBe(2);
            expect(config.machine_config.mem_size_mib).toBe(512);
        });
        it("creates valid CreateVMRequest", () => {
            const config = makeDefaultFirecrackerConfig({
                kernelPath: "/kernel",
                rootfsPath: "/rootfs",
            });
            const request = new CreateVMRequest({
                id: "test-vm",
                config,
            });
            expect(request.id).toBe("test-vm");
            expect(request.config).toEqual(config);
        });
    });
    describe("Service Operations", () => {
        const TestLayer = Layer.merge(FirecrackerServiceLive, NetworkSetupServiceLive);
        it.effect("gets binary path", () => Effect.gen(function* () {
            const service = yield* FirecrackerService;
            const result = yield* service.getBinaryPath().pipe(Effect.either);
            // Binary likely not found in test environment
            expect(result._tag).toBe("Left");
            if (result._tag === "Left") {
                expect(result.left._tag).toBe("FirecrackerBinaryNotFoundError");
            }
        }).pipe(Effect.provide(TestLayer)));
        it.effect("lists VMs when empty", () => Effect.gen(function* () {
            const service = yield* FirecrackerService;
            const vms = yield* service.listVMs();
            expect(vms).toHaveLength(0);
        }).pipe(Effect.provide(TestLayer)));
        it.effect("fails to get non-existent VM", () => Effect.gen(function* () {
            const service = yield* FirecrackerService;
            const result = yield* service.getVM("non-existent").pipe(Effect.either);
            expect(result._tag).toBe("Left");
            if (result._tag === "Left") {
                expect(result.left._tag).toBe("VMNotFoundError");
                expect(result.left.vmId).toBe("non-existent");
            }
        }).pipe(Effect.provide(TestLayer)));
        it.effect("fails to stop non-existent VM", () => Effect.gen(function* () {
            const service = yield* FirecrackerService;
            const request = new StopVMRequest({ id: "non-existent" });
            const result = yield* service.stopVM(request).pipe(Effect.either);
            expect(result._tag).toBe("Left");
            if (result._tag === "Left") {
                expect(result.left._tag).toBe("VMNotFoundError");
            }
        }).pipe(Effect.provide(TestLayer)));
    });
    describe("NetworkSetupService", () => {
        it.effect("checks permissions", () => Effect.gen(function* () {
            const service = yield* NetworkSetupService;
            const hasPerms = yield* service.checkPermissions();
            // In CI/test environment, likely false
            expect(typeof hasPerms).toBe("boolean");
        }).pipe(Effect.provide(NetworkSetupServiceLive)));
    });
});
//# sourceMappingURL=firecracker.test.js.map