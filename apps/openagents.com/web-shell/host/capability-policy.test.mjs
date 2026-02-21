import test from "node:test";
import assert from "node:assert/strict";

import {
  evaluateGpuCapability,
  isLinuxDesktopUserAgent,
  parseGpuModeOverride,
} from "./capability-policy.js";

test("parseGpuModeOverride supports explicit override values", () => {
  assert.equal(parseGpuModeOverride("?oa_gpu_mode=webgpu"), "webgpu");
  assert.equal(parseGpuModeOverride("?oa_gpu_mode=webgl2"), "webgl2");
  assert.equal(parseGpuModeOverride("?oa_gpu_mode=webgl"), "webgl2");
  assert.equal(parseGpuModeOverride("?oa_gpu_mode=limited"), "limited");
  assert.equal(parseGpuModeOverride("?oa_gpu_mode=unknown"), "auto");
  assert.equal(parseGpuModeOverride(""), "auto");
});

test("isLinuxDesktopUserAgent excludes Android", () => {
  assert.equal(
    isLinuxDesktopUserAgent(
      "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
    ),
    true,
  );
  assert.equal(
    isLinuxDesktopUserAgent("Mozilla/5.0 (Linux; Android 14; Pixel 8)"),
    false,
  );
});

test("auto mode prefers WebGPU outside Linux desktop policy", () => {
  const decision = evaluateGpuCapability({
    modeOverride: "auto",
    hasWebGpu: true,
    hasWebGl2: true,
    userAgent:
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_3) AppleWebKit/605.1.15",
  });
  assert.deepEqual(decision, {
    mode: "webgpu",
    reason: "auto_webgpu_supported",
  });
});

test("auto mode falls back to WebGL2 on Linux desktop policy", () => {
  const decision = evaluateGpuCapability({
    modeOverride: "auto",
    hasWebGpu: true,
    hasWebGl2: true,
    userAgent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
  });
  assert.deepEqual(decision, {
    mode: "webgl2",
    reason: "auto_webgl2_linux_reliability_policy",
  });
});

test("forced webgpu mode falls back deterministically when unavailable", () => {
  const decision = evaluateGpuCapability({
    modeOverride: "webgpu",
    hasWebGpu: false,
    hasWebGl2: true,
    userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)",
  });
  assert.deepEqual(decision, {
    mode: "webgl2",
    reason: "forced_webgpu_fallback_webgl2",
  });
});

test("limited mode is selected when no supported backend exists", () => {
  const decision = evaluateGpuCapability({
    modeOverride: "auto",
    hasWebGpu: false,
    hasWebGl2: false,
    userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
  });
  assert.deepEqual(decision, {
    mode: "limited",
    reason: "auto_no_supported_backend",
  });
});
