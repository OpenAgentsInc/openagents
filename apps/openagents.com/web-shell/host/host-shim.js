import init from "./openagents_web_shell.js";
import { evaluateBuildSkew, parseCompatibilityManifest } from "./update-policy.js";
import {
  evaluateGpuCapability,
  parseGpuModeOverride,
} from "./capability-policy.js";

const STATUS_ID = "openagents-web-shell-status";
const CURRENT_BUILD_ID = "__OA_BUILD_ID__";
const BUILD_SKEW_PROMOTION_TIMEOUT_MS = 4_000;

function detectWebGl2Support() {
  try {
    const canvas = document.createElement("canvas");
    const context =
      canvas.getContext("webgl2") ||
      canvas.getContext("experimental-webgl2");
    return Boolean(context);
  } catch (error) {
    console.warn("webgl2 capability probe failed", error);
    return false;
  }
}

function setStatus(text, isError = false) {
  const status = document.getElementById(STATUS_ID);
  if (!status) return;
  status.textContent = text;
  status.style.color = isError ? "#f87171" : "#cbd5e1";
}

async function fetchManifest() {
  try {
    const response = await fetch("/manifest.json", {
      cache: "no-store",
      headers: {
        "cache-control": "no-cache",
      },
    });

    if (!response.ok) {
      return null;
    }

    return await response.json();
  } catch (error) {
    console.warn("manifest fetch failed", error);
    return null;
  }
}

function waitForControllerChange(timeoutMs) {
  return new Promise((resolve) => {
    let done = false;

    const finish = (value) => {
      if (done) return;
      done = true;
      resolve(value);
    };

    const timeout = setTimeout(() => finish(false), timeoutMs);

    navigator.serviceWorker.addEventListener(
      "controllerchange",
      () => {
        clearTimeout(timeout);
        finish(true);
      },
      { once: true },
    );
  });
}

function waitForInstallingWorker(registration) {
  const installing = registration.installing;
  if (!installing) {
    return Promise.resolve(null);
  }

  return new Promise((resolve) => {
    const onStateChange = () => {
      if (installing.state === "installed") {
        installing.removeEventListener("statechange", onStateChange);
        resolve(registration.waiting ?? null);
      }
      if (installing.state === "redundant") {
        installing.removeEventListener("statechange", onStateChange);
        resolve(null);
      }
    };

    installing.addEventListener("statechange", onStateChange);
    onStateChange();
  });
}

async function promoteUpdatedWorker(registration) {
  if (!registration || !("serviceWorker" in navigator)) {
    return false;
  }

  await registration.update().catch((error) => {
    console.warn("service worker update check failed", error);
  });

  let waiting = registration.waiting ?? null;
  if (!waiting) {
    waiting = await waitForInstallingWorker(registration);
  }

  if (!waiting) {
    return false;
  }

  waiting.postMessage({ type: "SKIP_WAITING" });
  return await waitForControllerChange(BUILD_SKEW_PROMOTION_TIMEOUT_MS);
}

async function enforceBuildCompatibility(registration) {
  const manifest = await fetchManifest();
  if (!manifest) {
    return false;
  }

  const parsed = parseCompatibilityManifest(manifest);
  const decision = evaluateBuildSkew({
    currentBuildId: CURRENT_BUILD_ID,
    serverBuildId: parsed.buildId,
    minClientBuildId: parsed.minClientBuildId,
    maxClientBuildId: parsed.maxClientBuildId,
  });

  if (!decision.skewDetected) {
    return false;
  }

  setStatus(
    `Build skew detected (${decision.reason}). Refreshing pinned assets...`,
  );
  const promoted = await promoteUpdatedWorker(registration);
  if (promoted) {
    window.location.reload();
    return true;
  }

  setStatus(
    "Build compatibility check failed. Hard refresh to load a compatible bundle.",
    true,
  );
  return true;
}

function registerServiceWorker() {
  if (!("serviceWorker" in navigator)) {
    return Promise.resolve(null);
  }

  const url = `/sw.js?build=${encodeURIComponent(CURRENT_BUILD_ID)}`;
  return navigator.serviceWorker
    .register(url, { updateViaCache: "none" })
    .catch((error) => {
      console.warn("service worker registration failed", error);
      return null;
    });
}

async function boot() {
  const modeOverride = parseGpuModeOverride(window.location.search);
  const capability = evaluateGpuCapability({
    modeOverride,
    hasWebGpu: Boolean(navigator.gpu),
    hasWebGl2: detectWebGl2Support(),
    userAgent: navigator.userAgent || "",
  });

  window.__OA_GPU_MODE__ = capability.mode;
  window.__OA_GPU_REASON__ = capability.reason;

  if (capability.mode === "limited") {
    setStatus(
      "Boot error: this browser lacks required WebGPU/WebGL2 capability.",
      true,
    );
    console.error("openagents web shell capability failure", capability);
    return;
  }

  setStatus(
    `Boot: loading wasm entrypoint (${capability.mode}, ${capability.reason})`,
  );
  const registration = await registerServiceWorker();
  const blockedForSkew = await enforceBuildCompatibility(registration);
  if (blockedForSkew) {
    return;
  }

  await init("./openagents_web_shell_bg.wasm");
  setStatus("Boot: wasm initialized");
}

boot().catch((error) => {
  setStatus(`Boot error: ${String(error)}`, true);
  console.error("openagents web shell bootstrap failed", error);
});
