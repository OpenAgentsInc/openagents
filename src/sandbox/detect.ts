import { Effect, Layer } from "effect";
import { ContainerBackendTag, type ContainerBackend } from "./backend.js";
import { ContainerError } from "./schema.js";
import { macOSContainerLive } from "./macos-container.js";
import { dockerBackendLive } from "./docker.js";

// ─────────────────────────────────────────────────────────────────────────────
// NoOp Backend (when no container runtime is available)
// ─────────────────────────────────────────────────────────────────────────────

const noopBackend: ContainerBackend = {
  name: "none",
  isAvailable: () => Effect.succeed(false),
  run: () =>
    Effect.fail(
      new ContainerError("not_available", "No container runtime available"),
    ),
  build: () =>
    Effect.fail(
      new ContainerError("not_available", "No container runtime available"),
    ),
};

// ─────────────────────────────────────────────────────────────────────────────
// Auto-detection
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Detect and return the best available container backend.
 *
 * Priority:
 * 1. macOS Container (if on macOS 26+ with `container` CLI)
 * 2. Docker (all platforms where docker CLI is available)
 * 3. (Future: Seatbelt, etc.)
 * 3. NoOp backend (no sandboxing available)
 */
export const detectBackend = Effect.gen(function* () {
  const platformOverride = process.env.OA_SANDBOX_PLATFORM;
  const platform = platformOverride ?? process.platform;

  // Try macOS Container
  if (platform === "darwin") {
    const macosBackend = yield* Effect.provide(
      Effect.gen(function* () {
        const backend = yield* ContainerBackendTag;
        const available = yield* backend.isAvailable();
        return available ? backend : null;
      }),
      macOSContainerLive,
    );
    if (macosBackend) {
      return macosBackend;
    }
  }

  // Try Docker
  const dockerBackend = yield* Effect.provide(
    Effect.gen(function* () {
      const backend = yield* ContainerBackendTag;
      const available = yield* backend.isAvailable();
      return available ? backend : null;
    }),
    dockerBackendLive,
  );
  if (dockerBackend) {
    return dockerBackend;
  }

  // TODO: Add Seatbelt backend check here

  // Fallback to noop
  return noopBackend;
});

/**
 * Layer that auto-detects the best backend.
 * Use this when you want automatic backend selection.
 */
export const autoDetectLayer = Layer.effect(ContainerBackendTag, detectBackend);
