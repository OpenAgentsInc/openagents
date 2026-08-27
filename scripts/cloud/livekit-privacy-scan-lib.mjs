import { isUtf8 } from "node:buffer";
import { createHash, createPrivateKey } from "node:crypto";
import { lstatSync, readdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";

export const PRIVACY_SCAN_SCHEMA = "openagents.sarah.livekit_privacy_scan.v1";
export const PRIVACY_SCOPES = Object.freeze([
  "packaged_omega",
  "packaged_clients",
  "pods",
  "logs",
  "redis",
  "object_storage",
  "traces",
  "crash_artifacts",
]);
export const PRIVACY_SCOPE_EXPORT_SCHEMA = "openagents.sarah.livekit_privacy_scope_export.v1";
export const PRIVACY_SCOPE_MANIFEST = "_scope-manifest.json";

const CLIENT_SCOPES = new Set(["packaged_omega", "packaged_clients"]);
const WORKER_SCOPES = new Set(["pods"]);
const OPENAI_KEY_FORBIDDEN_SCOPES = new Set(PRIVACY_SCOPES.filter((scope) => scope !== "pods"));
const RETENTION_SCOPES = new Set([
  "pods",
  "logs",
  "redis",
  "object_storage",
  "traces",
  "crash_artifacts",
]);
const RAW_MEDIA_EXTENSIONS = new Set([
  ".aac",
  ".flac",
  ".m4a",
  ".mp3",
  ".ogg",
  ".opus",
  ".pcm",
  ".wav",
  ".webm",
]);
const TRANSCRIPT_NAME = /(?:^|[._-])(?:caption|captions|transcript|transcription)(?:[._-]|$)/iu;
const TRANSCRIPT_PAYLOAD =
  /"(?:caption|captions|transcript|transcription|recognized_text)"\s*:\s*"(?:[^"\\]|\\.)+"/iu;
const PRIVATE_KEY_LABELS = Object.freeze([
  "PRIVATE KEY",
  "EC PRIVATE KEY",
  "OPENSSH PRIVATE KEY",
  "RSA PRIVATE KEY",
]);
const MEDIA_SIGNATURES = Object.freeze([
  Buffer.from("OggS"),
  Buffer.from("fLaC"),
  Buffer.from("ID3"),
  Buffer.from([0x1a, 0x45, 0xdf, 0xa3]),
]);

const sha256 = (value) => createHash("sha256").update(value).digest("hex");

const extension = (name) => {
  const match = /(\.[^.]+)$/u.exec(name);
  return match?.[1]?.toLowerCase() ?? "";
};

const isWave = (bytes) =>
  bytes.length >= 12 &&
  bytes.subarray(0, 4).equals(Buffer.from("RIFF")) &&
  bytes.subarray(8, 12).equals(Buffer.from("WAVE"));

const contains = (bytes, needle) => needle.length > 0 && bytes.indexOf(needle) >= 0;

const startsWith = (bytes, prefix) =>
  bytes.length >= prefix.length && bytes.subarray(0, prefix.length).equals(prefix);

const containsParseablePrivateKey = (bytes) => {
  if (bytes.includes(0) || !isUtf8(bytes)) return false;
  const text = bytes.toString("utf8");
  for (const label of PRIVATE_KEY_LABELS) {
    const begin = `-----BEGIN ${label}-----`;
    const end = `-----END ${label}-----`;
    let searchFrom = 0;
    while (searchFrom < text.length) {
      const beginAt = text.indexOf(begin, searchFrom);
      if (beginAt < 0) break;
      const endAt = text.indexOf(end, beginAt + begin.length);
      if (endAt < 0) break;
      const candidate = text.slice(beginAt, endAt + end.length);
      try {
        createPrivateKey(candidate);
        return true;
      } catch {
        // Documentation and binary string tables often contain PEM delimiters.
      }
      searchFrom = beginAt + begin.length;
    }
  }
  return false;
};

const IMMUTABLE_POD_IMAGE_PAYLOAD = /^immutable-image\/sha256\/[0-9a-f]{64}\/[^/]+\/.+/u;

const classifyRetainedPayload = (scope, displayPath) =>
  scope !== "pods" || !IMMUTABLE_POD_IMAGE_PAYLOAD.test(displayPath);

const exactKeys = (value, keys) =>
  value !== null &&
  typeof value === "object" &&
  !Array.isArray(value) &&
  JSON.stringify(Object.keys(value).toSorted()) === JSON.stringify([...keys].toSorted());

const enumerate = (inputPath, maximumObjectBytes, scope, sourceBaseRevision, observedAt) => {
  const root = resolve(inputPath);
  const rootStat = lstatSync(root);
  if (rootStat.isSymbolicLink()) {
    throw new Error("scope input cannot be a symbolic link");
  }
  if (!rootStat.isDirectory()) {
    throw new Error("scope input must be a directory with a closed export manifest");
  }
  const canonicalRoot = realpathSync(root);
  const manifestPath = join(canonicalRoot, PRIVACY_SCOPE_MANIFEST);
  const manifestStat = lstatSync(manifestPath);
  if (!manifestStat.isFile() || manifestStat.isSymbolicLink()) {
    throw new Error("scope export manifest must be a regular file");
  }
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch (error) {
    throw new Error("scope export manifest is not readable JSON", { cause: error });
  }
  const manifestKeys = [
    "schemaVersion",
    "scope",
    "sourceBaseRevision",
    "collectionMode",
    "complete",
    "startedAt",
    "completedAt",
    "objectCount",
    "byteCount",
  ];
  if (!exactKeys(manifest, manifestKeys)) {
    throw new Error("scope export manifest fields do not match the closed schema");
  }
  if (
    manifest.schemaVersion !== PRIVACY_SCOPE_EXPORT_SCHEMA ||
    manifest.scope !== scope ||
    manifest.sourceBaseRevision !== sourceBaseRevision ||
    manifest.collectionMode !== "read_only" ||
    manifest.complete !== true
  ) {
    throw new Error("scope export manifest is unavailable or bound to the wrong source");
  }
  const startedAt = Date.parse(manifest.startedAt);
  const completedAt = Date.parse(manifest.completedAt);
  const scanObservedAt = Date.parse(observedAt);
  if (
    !Number.isFinite(startedAt) ||
    !Number.isFinite(completedAt) ||
    !Number.isFinite(scanObservedAt) ||
    completedAt < startedAt ||
    completedAt - startedAt > 2 * 60 * 60_000 ||
    completedAt > scanObservedAt + 5 * 60_000 ||
    scanObservedAt - completedAt > 24 * 60 * 60_000
  ) {
    throw new Error("scope export capture window is invalid or stale");
  }
  const objects = [];
  const visit = (directory) => {
    const entries = readdirSync(directory, { withFileTypes: true }).toSorted((left, right) =>
      left.name.localeCompare(right.name),
    );
    for (const entry of entries) {
      if (directory === canonicalRoot && entry.name === PRIVACY_SCOPE_MANIFEST) continue;
      const absolutePath = join(directory, entry.name);
      const entryStat = lstatSync(absolutePath);
      if (entryStat.isSymbolicLink()) {
        throw new Error("scope input contains a symbolic link");
      }
      if (entryStat.isDirectory()) {
        visit(absolutePath);
        continue;
      }
      if (!entryStat.isFile()) {
        throw new Error("scope input contains a non-regular object");
      }
      const canonicalPath = realpathSync(absolutePath);
      if (canonicalPath !== canonicalRoot && !canonicalPath.startsWith(`${canonicalRoot}${sep}`)) {
        throw new Error("scope input object escapes its root");
      }
      const size = statSync(canonicalPath).size;
      if (size > maximumObjectBytes) {
        throw new Error("scope input object exceeds the maximum byte limit");
      }
      objects.push({
        absolutePath: canonicalPath,
        displayPath: relative(canonicalRoot, canonicalPath),
        size,
      });
    }
  };
  visit(canonicalRoot);
  if (objects.length === 0) throw new Error("scope input is empty");
  const byteCount = objects.reduce((total, object) => total + object.size, 0);
  if (
    !Number.isSafeInteger(manifest.objectCount) ||
    manifest.objectCount !== objects.length ||
    !Number.isSafeInteger(manifest.byteCount) ||
    manifest.byteCount !== byteCount
  ) {
    throw new Error("scope export manifest counts do not match its payload");
  }
  return { objects, startedAt, completedAt };
};

const scanObject = ({ bytes, displayPath, scope, secrets, canaries }) => {
  let findings = 0;
  let rawMediaObjects = 0;
  let transcriptObjects = 0;

  if (OPENAI_KEY_FORBIDDEN_SCOPES.has(scope) && contains(bytes, secrets.openAiKey)) findings += 1;
  if (contains(bytes, secrets.sarahPrivateKey)) findings += 1;
  if (
    (CLIENT_SCOPES.has(scope) || WORKER_SCOPES.has(scope)) &&
    containsParseablePrivateKey(bytes)
  ) {
    findings += 1;
  }
  if (RETENTION_SCOPES.has(scope)) {
    for (const canary of canaries) {
      if (contains(bytes, canary)) findings += 1;
    }
    if (classifyRetainedPayload(scope, displayPath)) {
      if (
        RAW_MEDIA_EXTENSIONS.has(extension(displayPath)) ||
        isWave(bytes) ||
        MEDIA_SIGNATURES.some((signature) => startsWith(bytes, signature))
      ) {
        rawMediaObjects = 1;
      }
      const text = bytes.toString("utf8");
      if (TRANSCRIPT_NAME.test(displayPath) || TRANSCRIPT_PAYLOAD.test(text)) {
        transcriptObjects = 1;
      }
    }
  }
  return { findings, rawMediaObjects, transcriptObjects };
};

export function scanSarahLiveKitPrivacy({
  scopeInputs,
  openAiKey,
  sarahPrivateKey,
  canaries,
  sourceBaseRevision,
  observedAt = new Date().toISOString(),
  maximumObjectBytes = 256 * 1024 * 1024,
}) {
  if (!/^[0-9a-f]{40}$/u.test(sourceBaseRevision)) {
    throw new Error("sourceBaseRevision must be a full Git revision");
  }
  if (!Buffer.isBuffer(openAiKey) || openAiKey.length < 20) {
    throw new Error("OpenAI key evidence is missing or too short");
  }
  if (!Buffer.isBuffer(sarahPrivateKey) || sarahPrivateKey.length < 32) {
    throw new Error("Sarah private-key evidence is missing or too short");
  }
  if (!Array.isArray(canaries) || canaries.length === 0) {
    throw new Error("at least one private retention canary is required");
  }
  if (canaries.some((canary) => !Buffer.isBuffer(canary) || canary.length < 12)) {
    throw new Error("private retention canaries must contain at least 12 bytes");
  }
  const inputScopes = Object.keys(scopeInputs).toSorted();
  if (JSON.stringify(inputScopes) !== JSON.stringify([...PRIVACY_SCOPES].toSorted())) {
    throw new Error("scope inputs must cover exactly the eight required privacy scopes");
  }

  const secrets = { openAiKey, sarahPrivateKey };
  const scopeResults = [];
  let findings = 0;
  let rawMediaObjects = 0;
  let transcriptObjects = 0;
  let earliestCaptureStart = Number.POSITIVE_INFINITY;
  let latestCaptureEnd = Number.NEGATIVE_INFINITY;
  for (const scope of PRIVACY_SCOPES) {
    const scopeExport = enumerate(
      scopeInputs[scope],
      maximumObjectBytes,
      scope,
      sourceBaseRevision,
      observedAt,
    );
    const { objects } = scopeExport;
    earliestCaptureStart = Math.min(earliestCaptureStart, scopeExport.startedAt);
    latestCaptureEnd = Math.max(latestCaptureEnd, scopeExport.completedAt);
    const evidenceHash = createHash("sha256");
    let bytesScanned = 0;
    let scopeFindings = 0;
    let scopeRawMediaObjects = 0;
    let scopeTranscriptObjects = 0;
    for (const object of objects) {
      const bytes = readFileSync(object.absolutePath);
      bytesScanned += bytes.length;
      evidenceHash.update(object.displayPath);
      evidenceHash.update("\0");
      evidenceHash.update(sha256(bytes));
      evidenceHash.update("\0");
      const result = scanObject({
        bytes,
        displayPath: object.displayPath,
        scope,
        secrets,
        canaries,
      });
      scopeFindings += result.findings;
      scopeRawMediaObjects += result.rawMediaObjects;
      scopeTranscriptObjects += result.transcriptObjects;
    }
    if (bytesScanned === 0) throw new Error("scope input contains no evidence bytes");
    findings += scopeFindings;
    rawMediaObjects += scopeRawMediaObjects;
    transcriptObjects += scopeTranscriptObjects;
    scopeResults.push({
      scope,
      state: "complete",
      objectCount: objects.length,
      bytesScanned,
      evidenceDigest: `sha256:${evidenceHash.digest("hex")}`,
      findings: scopeFindings,
      rawMediaObjects: scopeRawMediaObjects,
      transcriptObjects: scopeTranscriptObjects,
    });
  }
  if (latestCaptureEnd - earliestCaptureStart > 2 * 60 * 60_000) {
    throw new Error("scope exports do not share one bounded capture window");
  }
  const outcome =
    findings === 0 && rawMediaObjects === 0 && transcriptObjects === 0 ? "passed" : "failed";
  return {
    schemaVersion: PRIVACY_SCAN_SCHEMA,
    sourceBaseRevision,
    observedAt,
    outcome,
    results: {
      scopes: [...PRIVACY_SCOPES],
      forbiddenPatternCount: 3 + canaries.length,
      findings,
      rawMediaObjects,
      transcriptObjects,
      scopeResults,
    },
  };
}

export function unavailableSarahLiveKitPrivacyResult({
  sourceBaseRevision,
  observedAt = new Date().toISOString(),
  unavailableScope,
}) {
  return {
    schemaVersion: PRIVACY_SCAN_SCHEMA,
    sourceBaseRevision,
    observedAt,
    outcome: "unavailable",
    results: {
      scopes: [...PRIVACY_SCOPES],
      unavailableScopes: unavailableScope ? [unavailableScope] : [...PRIVACY_SCOPES],
    },
  };
}
