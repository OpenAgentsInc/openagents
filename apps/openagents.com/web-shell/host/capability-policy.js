const LINUX_DESKTOP_PATTERN = /Linux/i;
const ANDROID_PATTERN = /Android/i;
const OVERRIDE_KEY = "oa_gpu_mode";

export function parseGpuModeOverride(search) {
  if (typeof search !== "string" || search.length === 0) {
    return "auto";
  }

  const query = search.startsWith("?") ? search.slice(1) : search;
  const params = new URLSearchParams(query);
  const raw = (params.get(OVERRIDE_KEY) ?? "").trim().toLowerCase();

  if (raw === "webgpu") return "webgpu";
  if (raw === "webgl2" || raw === "webgl") return "webgl2";
  if (raw === "limited") return "limited";
  return "auto";
}

export function isLinuxDesktopUserAgent(userAgent) {
  if (typeof userAgent !== "string" || userAgent.length === 0) {
    return false;
  }
  return LINUX_DESKTOP_PATTERN.test(userAgent) && !ANDROID_PATTERN.test(userAgent);
}

export function evaluateGpuCapability({
  modeOverride = "auto",
  hasWebGpu = false,
  hasWebGl2 = false,
  userAgent = "",
}) {
  const override = (modeOverride || "auto").toLowerCase();
  const linuxDesktop = isLinuxDesktopUserAgent(userAgent);

  if (override === "limited") {
    return { mode: "limited", reason: "forced_limited_mode" };
  }

  if (override === "webgpu") {
    if (hasWebGpu) {
      return { mode: "webgpu", reason: "forced_webgpu_mode" };
    }
    if (hasWebGl2) {
      return { mode: "webgl2", reason: "forced_webgpu_fallback_webgl2" };
    }
    return { mode: "limited", reason: "forced_webgpu_no_supported_backend" };
  }

  if (override === "webgl2") {
    if (hasWebGl2) {
      return { mode: "webgl2", reason: "forced_webgl2_mode" };
    }
    if (hasWebGpu) {
      return { mode: "webgpu", reason: "forced_webgl2_fallback_webgpu" };
    }
    return { mode: "limited", reason: "forced_webgl2_no_supported_backend" };
  }

  if (hasWebGpu && !linuxDesktop) {
    return { mode: "webgpu", reason: "auto_webgpu_supported" };
  }

  if (hasWebGl2) {
    const reason = linuxDesktop
      ? "auto_webgl2_linux_reliability_policy"
      : "auto_webgl2_fallback";
    return { mode: "webgl2", reason };
  }

  return { mode: "limited", reason: "auto_no_supported_backend" };
}
