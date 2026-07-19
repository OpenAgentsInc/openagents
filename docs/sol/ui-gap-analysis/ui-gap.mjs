#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import { readFile, stat, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { homedir, tmpdir } from "node:os"
import { dirname, isAbsolute, join, relative, resolve } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"

export const EVIDENCE_SCHEMA_VERSION = "openagents.ui-gap-evidence.v1"
export const CONFIG_SCHEMA_VERSION = "openagents.ui-gap-config.v1"

const scriptDirectory = dirname(fileURLToPath(import.meta.url))

function now() {
  return new Date().toISOString()
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex")
}

function parseArguments(argv) {
  const options = { _: [], roots: [], command: [] }
  let index = 0
  while (index < argv.length) {
    const value = argv[index]
    if (value === "--") {
      options.command = argv.slice(index + 1)
      break
    }
    if (!value.startsWith("--")) {
      options._.push(value)
      index += 1
      continue
    }
    const equal = value.indexOf("=")
    const key = value.slice(2, equal > 0 ? equal : undefined)
    const next = equal > 0 ? value.slice(equal + 1) : argv[index + 1]
    if (key === "root") {
      if (equal < 0) index += 1
      options.roots.push(next)
    } else if (next !== undefined && (equal > 0 || !next.startsWith("--"))) {
      if (equal < 0) index += 1
      options[key] = next
    } else {
      options[key] = true
    }
    index += 1
  }
  return options
}

function rootMap(entries) {
  const result = new Map()
  for (const entry of entries ?? []) {
    const separator = entry.indexOf("=")
    if (separator < 1) throw new Error(`Invalid --root value: ${entry}`)
    result.set(entry.slice(0, separator), resolve(entry.slice(separator + 1)))
  }
  return result
}

function execute(cwd, argv, options = {}) {
  const result = spawnSync(argv[0], argv.slice(1), {
    cwd,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    ...options,
  })
  if (result.status !== 0) {
    throw new Error(`${argv.join(" ")} failed: ${(result.stderr || result.stdout || "").trim()}`)
  }
  return typeof result.stdout === "string" ? result.stdout.trim() : result.stdout
}

export function globToRegularExpression(glob) {
  let expression = "^"
  for (let index = 0; index < glob.length; index += 1) {
    const character = glob[index]
    if (character === "*") {
      if (glob[index + 1] === "*") {
        expression += ".*"
        index += 1
      } else {
        expression += "[^/]*"
      }
    } else if (character === "?") {
      expression += "[^/]"
    } else {
      expression += character.replace(/[|\\{}()[\]^$+?.]/g, "\\$&")
    }
  }
  return new RegExp(`${expression}$`)
}

function matchesAny(path, globs) {
  return globs.some((glob) => globToRegularExpression(glob).test(path))
}

function gitIdentity(root, publicRoot = "<REPO_ROOT>") {
  const commit = execute(root, ["git", "rev-parse", "HEAD"])
  const tree = execute(root, ["git", "rev-parse", "HEAD^{tree}"])
  const status = execute(root, ["git", "status", "--porcelain", "--untracked-files=no"])
  const [commitTime, ...subjectParts] = execute(root, ["git", "log", "-1", "--format=%cI%n%s"]).split("\n")
  return {
    root: publicRoot,
    commit,
    tree,
    dirty: status.length > 0,
    commitTime,
    subject: subjectParts.join("\n"),
  }
}

export function validateConfig(config) {
  const errors = []
  if (config?.schemaVersion !== CONFIG_SCHEMA_VERSION) errors.push(`schemaVersion must be ${CONFIG_SCHEMA_VERSION}`)
  if (!config?.analysisId) errors.push("analysisId is required")
  if (!Array.isArray(config?.targets) || config.targets.length < 2) errors.push("at least two targets are required")
  if (!Array.isArray(config?.probes) || config.probes.length === 0) errors.push("at least one probe is required")
  const targetIds = new Set()
  for (const target of config?.targets ?? []) {
    if (!target.id) errors.push("each target needs an id")
    if (targetIds.has(target.id)) errors.push(`duplicate target id: ${target.id}`)
    targetIds.add(target.id)
    if (!Array.isArray(target.corpus?.include) || target.corpus.include.length === 0) {
      errors.push(`${target.id ?? "target"} needs corpus.include`)
    }
    if (!/^[0-9a-f]{40}$/.test(target.expectedCommit ?? "")) errors.push(`${target.id} expectedCommit is invalid`)
    if (!/^[0-9a-f]{40}$/.test(target.expectedTree ?? "")) errors.push(`${target.id} expectedTree is invalid`)
  }
  const probeIds = new Set()
  for (const probe of config?.probes ?? []) {
    if (!probe.id || !probe.axis) errors.push("each probe needs id and axis")
    if (probeIds.has(probe.id)) errors.push(`duplicate probe id: ${probe.id}`)
    probeIds.add(probe.id)
    for (const target of config?.targets ?? []) {
      if (!probe.patterns?.[target.id]) errors.push(`${probe.id} has no pattern for ${target.id}`)
      try {
        new RegExp(probe.patterns?.[target.id] ?? "")
      } catch (error) {
        errors.push(`${probe.id} has an invalid pattern for ${target.id}: ${error.message}`)
      }
    }
  }
  return errors
}

async function loadConfig(path) {
  const config = JSON.parse(await readFile(resolve(path), "utf8"))
  const errors = validateConfig(config)
  if (errors.length > 0) throw new Error(errors.join("\n"))
  return config
}

function assertIdentity(target, identity, allowRevisionMismatch) {
  const mismatches = []
  if (identity.commit !== target.expectedCommit) mismatches.push(`commit ${identity.commit} != ${target.expectedCommit}`)
  if (identity.tree !== target.expectedTree) mismatches.push(`tree ${identity.tree} != ${target.expectedTree}`)
  if (mismatches.length > 0 && !allowRevisionMismatch) {
    throw new Error(`${target.id} revision mismatch: ${mismatches.join(", ")}`)
  }
  return mismatches
}

function toolVersion(argv) {
  try {
    const result = spawnSync(argv[0], argv.slice(1), { encoding: "utf8", maxBuffer: 1024 * 1024 })
    const text = `${result.stdout ?? ""}\n${result.stderr ?? ""}`.trim().split("\n")[0]
    return { available: result.status === 0, version: text || null }
  } catch (error) {
    return { available: false, version: null, error: error.message }
  }
}

async function writeJson(path, value) {
  const target = resolve(path)
  await writeFile(target, `${JSON.stringify(value, null, 2)}\n`)
  return target
}

export async function doctor(config, roots, allowRevisionMismatch = false) {
  const targets = []
  const limitations = []
  for (const target of config.targets) {
    const root = roots.get(target.id)
    if (!root) {
      targets.push({ id: target.id, available: false, error: `No --root was supplied for ${target.id}` })
      continue
    }
    try {
      const identity = gitIdentity(root, `<${target.id.toUpperCase()}_ROOT>`)
      const mismatches = assertIdentity(target, identity, allowRevisionMismatch)
      targets.push({ id: target.id, available: true, identity, mismatches })
    } catch (error) {
      targets.push({ id: target.id, available: false, error: error.message })
    }
  }
  if (process.platform !== "darwin") limitations.push("The runtime capture adapter is available only on macOS.")
  return {
    schemaVersion: EVIDENCE_SCHEMA_VERSION,
    kind: "doctor",
    generatedAt: now(),
    analysisId: config.analysisId,
    doctor: {
      platform: `${process.platform}-${process.arch}`,
      node: process.version,
      tools: {
        git: toolVersion(["git", "--version"]),
        swift: toolVersion(["swift", "--version"]),
        screencapture: { available: existsSync("/usr/sbin/screencapture"), version: null },
      },
      targets,
    },
    limitations,
  }
}

export async function scanTarget(config, target, root, allowRevisionMismatch = false) {
  const identity = gitIdentity(root, `<${target.id.toUpperCase()}_ROOT>`)
  const mismatches = assertIdentity(target, identity, allowRevisionMismatch)
  const tracked = execute(root, ["git", "ls-files", "-z"])
  const trackedFiles = tracked.split("\0").filter(Boolean)
  const includes = target.corpus.include
  const excludes = target.corpus.exclude ?? []
  const corpusPaths = trackedFiles
    .filter((path) => matchesAny(path, includes) && !matchesAny(path, excludes))
    .sort()
  const maxFileBytes = config.maxFileBytes ?? 2 * 1024 * 1024
  const fileRecords = []
  for (const path of corpusPaths) {
    const absolute = join(root, path)
    const metadata = await stat(absolute)
    if (!metadata.isFile() || metadata.size > maxFileBytes) continue
    const bytes = await readFile(absolute)
    if (bytes.includes(0)) continue
    fileRecords.push({ path, bytes, digest: sha256(bytes) })
  }
  const probes = []
  for (const probe of config.probes) {
    const pattern = probe.patterns[target.id]
    const matchingFiles = new Set()
    let matchCount = 0
    const samples = []
    for (const file of fileRecords) {
      const lines = file.bytes.toString("utf8").split("\n")
      for (let index = 0; index < lines.length; index += 1) {
        const matches = lines[index].match(new RegExp(pattern, "gi"))
        if (!matches) continue
        matchCount += matches.length
        matchingFiles.add(file.path)
        if (samples.length < (config.sampleLimit ?? 8)) {
          samples.push({
            path: file.path,
            line: index + 1,
            text: lines[index].trim().replace(/\s+/g, " ").slice(0, 240),
            sha256: file.digest,
          })
        }
      }
    }
    probes.push({ id: probe.id, axis: probe.axis, matchCount, fileCount: matchingFiles.size, samples })
  }
  const corpusDigest = sha256(fileRecords.map((file) => `${file.path}\0${file.digest}`).join("\n"))
  return {
    schemaVersion: EVIDENCE_SCHEMA_VERSION,
    kind: "source-scan",
    generatedAt: now(),
    analysisId: config.analysisId,
    targetId: target.id,
    identity,
    source: {
      trackedFiles: trackedFiles.length,
      corpusFiles: fileRecords.length,
      corpusBytes: fileRecords.reduce((total, file) => total + file.bytes.length, 0),
      corpusDigest,
      probes,
    },
    limitations: [
      "A source match is evidence for inspection. It is not proof that a user-visible capability works.",
      ...mismatches.map((mismatch) => `Revision mismatch allowed by the operator: ${mismatch}`),
    ],
  }
}

function appendTail(current, value, limit = 12000) {
  const next = `${current}${value}`
  return next.length > limit ? next.slice(next.length - limit) : next
}

function sanitizeRecordedText(value, cwd, publicCwd) {
  return value
    .split(cwd).join(publicCwd ?? "<COMMAND_ROOT>")
    .split(homedir()).join("<HOME>")
    .split(tmpdir()).join("<TEMP>")
    .replace(/\/(?:private\/)?tmp\/[A-Za-z0-9._/-]+/g, "<TEMP_PATH>")
    .replace(/\/var\/folders\/[A-Za-z0-9._/-]+/g, "<TEMP_PATH>")
}

export async function recordCommand({ label, cwd, publicCwd, argv, artifact, publicArtifact }) {
  if (!argv || argv.length === 0) throw new Error("record-command needs an argv after --")
  const startedAt = now()
  const start = Date.now()
  let stdoutTail = ""
  let stderrTail = ""
  const outcome = await new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(argv[0], argv.slice(1), { cwd, env: process.env })
    child.stdout.on("data", (chunk) => {
      process.stdout.write(chunk)
      stdoutTail = appendTail(stdoutTail, chunk.toString("utf8"))
    })
    child.stderr.on("data", (chunk) => {
      process.stderr.write(chunk)
      stderrTail = appendTail(stderrTail, chunk.toString("utf8"))
    })
    child.once("error", rejectPromise)
    child.once("close", (exitCode, signal) => resolvePromise({ exitCode, signal }))
  })
  let artifactRecord
  if (artifact) {
    const artifactPath = isAbsolute(artifact) ? artifact : resolve(cwd, artifact)
    if (existsSync(artifactPath)) {
      const bytes = await readFile(artifactPath)
      artifactRecord = { path: publicArtifact ?? `<ARTIFACT>/${artifactPath.split("/").at(-1)}`, sha256: sha256(bytes), bytes: bytes.length }
    }
  }
  return {
    schemaVersion: EVIDENCE_SCHEMA_VERSION,
    kind: "command-receipt",
    generatedAt: now(),
    command: {
      label,
      argv: argv.map((value) => sanitizeRecordedText(value, cwd, publicCwd)),
      cwd: publicCwd ?? "<COMMAND_ROOT>",
      startedAt,
      completedAt: now(),
      durationMs: Date.now() - start,
      exitCode: outcome.exitCode,
      signal: outcome.signal,
      stdoutTail: sanitizeRecordedText(stdoutTail, cwd, publicCwd),
      stderrTail: sanitizeRecordedText(stderrTail, cwd, publicCwd),
      ...(artifactRecord ? { artifact: artifactRecord } : {}),
    },
    limitations: outcome.exitCode === 0 ? [] : ["The command did not complete successfully."],
  }
}

function roleCounts(runtimeEvidence) {
  const counts = {}
  for (const node of runtimeEvidence?.runtime?.accessibility?.nodes ?? []) {
    counts[node.role] = (counts[node.role] ?? 0) + 1
  }
  return counts
}

function numericDelta(left, right, key) {
  const leftValue = left?.runtime?.visual?.[key]
  const rightValue = right?.runtime?.visual?.[key]
  return typeof leftValue === "number" && typeof rightValue === "number"
    ? Number((rightValue - leftValue).toFixed(6))
    : null
}

export function compareEvidence({ analysisId, leftSource, rightSource, leftRuntime, rightRuntime }) {
  const leftProbes = new Map((leftSource?.source?.probes ?? []).map((probe) => [probe.id, probe]))
  const rightProbes = new Map((rightSource?.source?.probes ?? []).map((probe) => [probe.id, probe]))
  const probeIds = [...new Set([...leftProbes.keys(), ...rightProbes.keys()])].sort()
  const sourceProbeDeltas = probeIds.map((id) => {
    const left = leftProbes.get(id)
    const right = rightProbes.get(id)
    const leftCount = left?.matchCount ?? 0
    const rightCount = right?.matchCount ?? 0
    return { id, axis: left?.axis ?? right?.axis ?? "unknown", left: leftCount, right: rightCount, delta: rightCount - leftCount }
  })
  const leftRoles = roleCounts(leftRuntime)
  const rightRoles = roleCounts(rightRuntime)
  const accessibilityRoleDeltas = {}
  for (const role of [...new Set([...Object.keys(leftRoles), ...Object.keys(rightRoles)])].sort()) {
    const left = leftRoles[role] ?? 0
    const right = rightRoles[role] ?? 0
    accessibilityRoleDeltas[role] = { left, right, delta: right - left }
  }
  return {
    schemaVersion: EVIDENCE_SCHEMA_VERSION,
    kind: "comparison",
    generatedAt: now(),
    analysisId,
    comparison: {
      leftTargetId: leftSource?.targetId ?? leftRuntime?.targetId ?? "left",
      rightTargetId: rightSource?.targetId ?? rightRuntime?.targetId ?? "right",
      sourceProbeDeltas,
      accessibilityRoleDeltas,
      visualDeltas: {
        meanLuma: numericDelta(leftRuntime, rightRuntime, "meanLuma"),
        lumaStandardDeviation: numericDelta(leftRuntime, rightRuntime, "lumaStandardDeviation"),
        lumaP10: numericDelta(leftRuntime, rightRuntime, "lumaP10"),
        lumaP90: numericDelta(leftRuntime, rightRuntime, "lumaP90"),
        edgeDensity: numericDelta(leftRuntime, rightRuntime, "edgeDensity"),
      },
    },
    limitations: [
      "A source-probe delta measures evidence density. It does not measure capability quality or completeness.",
      "A screenshot delta measures pixels. It does not measure usability.",
      "The accessibility comparison includes only nodes that the platform API exposed in the selected window state.",
      ...(leftRuntime?.runtime?.accessibility?.provider !== rightRuntime?.runtime?.accessibility?.provider
        ? ["The accessibility providers use different role namespaces. Direct role deltas are not parity measures."]
        : []),
    ],
  }
}

export function validateEvidence(evidence) {
  const errors = []
  if (evidence?.schemaVersion !== EVIDENCE_SCHEMA_VERSION) errors.push(`schemaVersion must be ${EVIDENCE_SCHEMA_VERSION}`)
  if (!["doctor", "source-scan", "command-receipt", "runtime-capture", "ocr-capture", "comparison", "gap-assessment"].includes(evidence?.kind)) {
    errors.push("kind is invalid")
  }
  if (Number.isNaN(Date.parse(evidence?.generatedAt ?? ""))) errors.push("generatedAt is invalid")
  if (evidence?.kind === "source-scan") {
    if (!evidence.analysisId || !evidence.targetId || !evidence.identity || !evidence.source) errors.push("source-scan fields are incomplete")
    if (!Array.isArray(evidence.source?.probes)) errors.push("source.probes must be an array")
  }
  if (evidence?.kind === "command-receipt" && !evidence.command) errors.push("command is required")
  if (evidence?.kind === "runtime-capture" && (!evidence.analysisId || !evidence.targetId || !evidence.runtime)) {
    errors.push("runtime-capture fields are incomplete")
  }
  if (evidence?.kind === "ocr-capture" && (!evidence.analysisId || !evidence.targetId || !evidence.ocr)) {
    errors.push("ocr-capture fields are incomplete")
  }
  if (evidence?.kind === "comparison" && (!evidence.analysisId || !evidence.comparison)) errors.push("comparison is required")
  if (evidence?.kind === "gap-assessment" && (!evidence.analysisId || !evidence.assessment)) errors.push("assessment is required")
  const serialized = JSON.stringify(evidence)
  if (/\/(?:Users|home)\/[^"\\]+/.test(serialized)) errors.push("evidence contains a local home path")
  if (/\/(?:private\/)?tmp\/[^"\\]+/.test(serialized) || /\/var\/folders\/[^"\\]+/.test(serialized)) {
    errors.push("evidence contains a local temporary path")
  }
  return errors
}

async function loadJson(path) {
  return JSON.parse(await readFile(resolve(path), "utf8"))
}

async function captureMacOS(config, targetId, options) {
  if (process.platform !== "darwin") throw new Error("capture-macos requires macOS")
  const outputDirectory = resolve(options["output-dir"])
  const swiftScript = join(scriptDirectory, "macos-ui-capture.swift")
  const argv = [swiftScript, "--output-dir", outputDirectory]
  if (options.pid) argv.push("--pid", String(options.pid))
  if (options.process) argv.push("--process", options.process)
  if (options["window-title"]) argv.push("--window-title", options["window-title"])
  if (options["max-depth"]) argv.push("--max-depth", String(options["max-depth"]))
  if (options["max-nodes"]) argv.push("--max-nodes", String(options["max-nodes"]))
  const result = spawnSync("swift", argv, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 })
  if (result.status !== 0) throw new Error(`macOS capture failed: ${(result.stderr || result.stdout).trim()}`)
  const captured = JSON.parse(result.stdout)
  const { limitations = [], ...runtime } = captured
  runtime.accessibility.provider = "macos-ax"
  return {
    schemaVersion: EVIDENCE_SCHEMA_VERSION,
    kind: "runtime-capture",
    generatedAt: now(),
    analysisId: config.analysisId,
    targetId,
    runtime,
    limitations,
  }
}

function captureOCR(config, targetId, options) {
  if (process.platform !== "darwin") throw new Error("capture-ocr requires macOS")
  const swiftScript = join(scriptDirectory, "macos-vision-ocr.swift")
  const result = spawnSync(
    "swift",
    [swiftScript, "--image", resolve(options.image), "--public-image", options["public-image"] ?? "window.png"],
    { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  )
  if (result.status !== 0) throw new Error(`OCR capture failed: ${(result.stderr || result.stdout).trim()}`)
  return {
    schemaVersion: EVIDENCE_SCHEMA_VERSION,
    kind: "ocr-capture",
    generatedAt: now(),
    analysisId: config.analysisId,
    targetId,
    ocr: JSON.parse(result.stdout),
    limitations: [
      "OCR text is an image observation. It does not prove that a control is accessible or interactive.",
      "Use only a disposable fixture because OCR can record visible editor and account text.",
    ],
  }
}

function usage() {
  return `Usage:
  node ui-gap.mjs doctor --config <path> --root <id>=<repo>... --out <json>
  node ui-gap.mjs scan-source --config <path> --root <id>=<repo>... --out-dir <dir>
  node ui-gap.mjs record-command --label <label> --cwd <dir> --public-cwd <label> --out <json> [--artifact <path> --public-artifact <label>] -- <command> [args...]
  node ui-gap.mjs capture-macos --config <path> --target <id> (--pid <pid>|--process <name>) --output-dir <dir> --out <json>
  node ui-gap.mjs capture-ocr --config <path> --target <id> --image <png> --public-image <label> --out <json>
  node ui-gap.mjs compare --config <path> --left-source <json> --right-source <json> [--left-runtime <json> --right-runtime <json>] --out <json>
  node ui-gap.mjs validate --file <json> [--file <json> is accepted one at a time]
`
}

async function main(argv) {
  const options = parseArguments(argv)
  const command = options._[0]
  if (!command || command === "help" || options.help) {
    process.stdout.write(usage())
    return
  }
  if (command === "validate") {
    const evidence = await loadJson(options.file)
    const errors = validateEvidence(evidence)
    if (errors.length > 0) throw new Error(errors.join("\n"))
    process.stdout.write(`${options.file}: valid\n`)
    return
  }
  if (command === "record-command") {
    const receipt = await recordCommand({
      label: options.label,
      cwd: resolve(options.cwd),
      publicCwd: options["public-cwd"],
      argv: options.command,
      artifact: options.artifact,
      publicArtifact: options["public-artifact"],
    })
    await writeJson(options.out, receipt)
    if (receipt.command.exitCode !== 0) process.exitCode = receipt.command.exitCode ?? 1
    return
  }
  const config = await loadConfig(options.config)
  if (command === "doctor") {
    const evidence = await doctor(config, rootMap(options.roots), Boolean(options["allow-revision-mismatch"]))
    await writeJson(options.out, evidence)
    return
  }
  if (command === "scan-source") {
    const roots = rootMap(options.roots)
    for (const target of config.targets) {
      const root = roots.get(target.id)
      if (!root) throw new Error(`No --root was supplied for ${target.id}`)
      const evidence = await scanTarget(config, target, root, Boolean(options["allow-revision-mismatch"]))
      await writeJson(join(resolve(options["out-dir"]), `${target.id}-source.json`), evidence)
    }
    return
  }
  if (command === "capture-macos") {
    const evidence = await captureMacOS(config, options.target, options)
    await writeJson(options.out, evidence)
    return
  }
  if (command === "capture-ocr") {
    const evidence = captureOCR(config, options.target, options)
    await writeJson(options.out, evidence)
    return
  }
  if (command === "compare") {
    const leftSource = await loadJson(options["left-source"])
    const rightSource = await loadJson(options["right-source"])
    const leftRuntime = options["left-runtime"] ? await loadJson(options["left-runtime"]) : undefined
    const rightRuntime = options["right-runtime"] ? await loadJson(options["right-runtime"]) : undefined
    const evidence = compareEvidence({ analysisId: config.analysisId, leftSource, rightSource, leftRuntime, rightRuntime })
    await writeJson(options.out, evidence)
    return
  }
  throw new Error(`Unknown command: ${command}\n${usage()}`)
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : ""
if (import.meta.url === invokedPath) {
  main(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`${error.stack ?? error.message}\n`)
    process.exitCode = 1
  })
}
