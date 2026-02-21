export function normalizeBuildId(input) {
  if (typeof input !== "string") return "";
  return input.trim();
}

export function compareBuildIds(a, b) {
  const left = normalizeBuildId(a);
  const right = normalizeBuildId(b);
  if (!left || !right) return 0;
  return left.localeCompare(right);
}

export function parseCompatibilityManifest(manifest) {
  if (!manifest || typeof manifest !== "object") {
    return {
      buildId: "",
      minClientBuildId: "",
      maxClientBuildId: "",
    };
  }

  const compatibility =
    manifest.compatibility && typeof manifest.compatibility === "object"
      ? manifest.compatibility
      : {};

  return {
    buildId: normalizeBuildId(manifest.buildId),
    minClientBuildId: normalizeBuildId(compatibility.minClientBuildId),
    maxClientBuildId: normalizeBuildId(compatibility.maxClientBuildId),
  };
}

export function evaluateBuildSkew({
  currentBuildId,
  serverBuildId,
  minClientBuildId,
  maxClientBuildId,
}) {
  const current = normalizeBuildId(currentBuildId);
  const server = normalizeBuildId(serverBuildId);
  const minClient = normalizeBuildId(minClientBuildId);
  const maxClient = normalizeBuildId(maxClientBuildId);

  if (!current) {
    return {
      skewDetected: true,
      reason: "missing_current_build",
    };
  }

  if (minClient && compareBuildIds(current, minClient) < 0) {
    return {
      skewDetected: true,
      reason: "below_min_client",
    };
  }

  if (maxClient && compareBuildIds(current, maxClient) > 0) {
    return {
      skewDetected: true,
      reason: "above_max_client",
    };
  }

  if (server && server !== current) {
    return {
      skewDetected: true,
      reason: "build_id_mismatch",
    };
  }

  return {
    skewDetected: false,
    reason: "none",
  };
}
