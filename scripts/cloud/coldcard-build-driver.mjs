#!/usr/bin/env node

/**
 * Coldcard artifact-witness build driver.
 *
 * Runs one pinned Coldcard MK4 firmware build inside an OpenAgents Cloud
 * managed-sandbox guest and emits an `openagents.artifact_witness_capture.v2`
 * capture describing what the build actually produced.
 *
 * This driver only ever REPORTS observations. It never asserts a verdict:
 * build outcome, symbol-inventory completeness and call-graph completeness are
 * derived by `evaluateArtifactWitness` in @openagentsinc/forensic-contract from
 * the observations below. The provenance block is filled in by the operator
 * harness from the live managed-sandbox receipts, because a guest cannot
 * attest to its own admission.
 *
 * Everything here is measured with the toolchain that performed the build:
 * `nm`, `objdump -dr`, `readelf`, the linker map, the `-E` preprocessor, and
 * the recorded compiler command inventory. Nothing is inferred from source
 * reading, and nothing is copied from a fixture.
 */

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {

  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import { pathToFileURL } from "node:url";

const DRIVER_REF = "driver.openagents.coldcard-build.v1";
const CAPTURE_SCHEMA = "openagents.artifact_witness_capture.v2";
const BOARD = "COLDCARD_MK4";
const BUILD_SUBDIR = `stm32/l-port/build-${BOARD}`;
const SOURCE_ROOT = "/opt/coldcard";

/** Objects whose call sites carry the RNG causal chain this witness is about. */
const WITNESSED_OBJECTS = [
  "rng.o",
  `boards/${BOARD}/rng.o`,
  "libngu/random.o",
  "extmod/modurandom.o",
];

const refuse = (reason, detail) => {
  process.stderr.write(`${JSON.stringify({ error: "coldcard_build_refused", reason, detail })}\n`);
  process.exit(1);
};

const sha256 = (buffer) => `sha256:${createHash("sha256").update(buffer).digest("hex")}`;
const digestOfFile = (path) => sha256(readFileSync(path));
const digestOfText = (text) => sha256(Buffer.from(text, "utf8"));

/**
 * Refs are `ForensicRef`, which admits only `[A-Za-z0-9._:/#-]` after the
 * first character. Build paths already satisfy that; anything else is folded
 * to `-` so a ref can never silently become a different ref.
 */
const ref = (...parts) => {
  const value = parts.join(".").replace(/[^A-Za-z0-9._:/#-]/g, "-");
  return /^[A-Za-z0-9]/.test(value) ? value : `x${value}`;
};

const run = (command, args, options = {}) =>
  spawnSync(command, args, { encoding: "utf8", maxBuffer: 512 * 1024 * 1024, ...options });

const mustRun = (command, args, options = {}) => {
  const result = run(command, args, options);
  if (result.status !== 0) {
    refuse("tool_failed", `${command} ${args.join(" ")} exited ${result.status}`);
  }
  return result.stdout;
};

const walk = (root, predicate) => {
  const found = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile() && predicate(path)) found.push(path);
    }
  };
  if (existsSync(root)) visit(root);
  return found;
};

/**
 * The one mutation the fault build applies: delete the approved hardware-TRNG
 * provider of `rng_get` that the fixed commit added to the board's rng.c.
 * Nothing else is touched, so a non-zero exit can only be caused by the
 * removal of that provider.
 */
const FAULT_TARGET = `stm32/${BOARD}/rng.c`;
const FAULT_PROVIDER_BLOCK = "uint32_t rng_get(void)\n{\n    return rng_get_or_fault();\n}\n";
const FAULT_MUTATION_REF = "mutation.coldcard.board-rng-get-provider-removed";

const applyFaultMutation = (tree) => {
  const path = join(tree, FAULT_TARGET);
  const before = readFileSync(path, "utf8");
  if (!before.includes(FAULT_PROVIDER_BLOCK)) {
    refuse("fault_provider_block_absent", FAULT_TARGET);
  }
  writeFileSync(path, before.replace(FAULT_PROVIDER_BLOCK, ""));
  return {
    faultMutationRef: FAULT_MUTATION_REF,
    beforeDigest: digestOfText(before),
    afterDigest: digestOfFile(path),
  };
};

/**
 * Recover the exact compiler invocation for one translation unit from the
 * `V=1` build log. The log is the only place the real flags exist; guessing
 * them would make the preprocessed artifact describe a different build.
 */
export const compilerCommandFor = (log, objectPath) => {
  for (const line of log.split("\n")) {
    if (!line.includes("-c ") || !line.includes("arm-none-eabi-gcc")) continue;
    const match = / -o (\S+)/.exec(line);
    if (match !== null && match[1] === objectPath) return line.trim();
  }
  return undefined;
};

const compilerCommandInventory = (log) =>
  log
    .split("\n")
    .filter((line) => /arm-none-eabi-(gcc|ld|ar|objcopy|nm)\b/.test(line))
    .map((line) => line.trim())
    .join("\n");

/**
 * Re-run a recorded compile command as a preprocess-only invocation.
 *
 * The recorded line is replayed through a shell rather than word-split, because
 * the real command carries shell-quoted defines such as
 * `-DSTM32_HAL_H='<stm32l4xx_hal.h>'`. Word-splitting them silently produces a
 * DIFFERENT configuration, which would make the preprocessed artifact describe
 * a build that never happened.
 */
export const preprocess = (command, cwd, outputPath, extraFlags) => {
  const preprocessCommand = `${command
    .replace(/ -c(?= )/, " ")
    .replace(/ -MD(?= )/, " ")
    .replace(/ -o \S+/, "")} ${extraFlags.join(" ")} -E`;
  const result = run("/bin/sh", ["-c", preprocessCommand], { cwd });
  if (result.status !== 0) return undefined;
  writeFileSync(outputPath, result.stdout);
  return outputPath;
};

/**
 * Read a macro's post-configuration value out of the preprocessor's own macro
 * dump. This is the value the compiler used, not the value a reader of the
 * header would guess.
 */
export const macroValue = (macroDump, macroName) => {
  const line = macroDump.split("\n").find((entry) => entry.startsWith(`#define ${macroName} `));
  return line === undefined ? undefined : line.slice(`#define ${macroName} `.length).trim();
};

/**
 * Attribute every relocation-bearing call site to the function that contains
 * it, by walking `objdump -dr` output and tracking the current symbol header.
 * Indirect branches through a register are counted separately: they are the
 * call sites this method cannot resolve, and a non-reachability claim is not
 * provable while that count is non-zero.
 */
export const callSites = (objdumpText) => {
  const edges = [];
  let unresolvedIndirect = 0;
  let current;
  for (const line of objdumpText.split("\n")) {
    const header = /^[0-9a-f]+ <([^>]+)>:$/.exec(line);
    if (header !== null) {
      current = header[1];
      continue;
    }
    const reloc = /R_ARM_(?:THM_)?(?:CALL|JUMP24|PC24|THM_JUMP24)\s+(\S+)/.exec(line);
    if (reloc !== null && current !== undefined) {
      edges.push({ from: current, to: reloc[1].split("+")[0] });
      continue;
    }
    if (/\b(?:blx|bx)\s+r(?:[0-9]|1[0-2])\b/.test(line)) unresolvedIndirect += 1;
  }
  return { edges, unresolvedIndirect };
};

/**
 * Which object actually provided a linked symbol, read from the linker map.
 * The map is the link's own record of the decision, so it cannot disagree with
 * the firmware that was produced.
 */
export const linkedProviderOf = (mapText, symbolName) => {
  const lines = mapText.split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    const section = new RegExp(`^\\s*\\.text\\.${symbolName}\\s`).exec(lines[index]);
    if (section === null) continue;
    const provider = /(\S+\.o)\s*$/.exec(lines[index]) ?? /(\S+\.o)\s*$/.exec(lines[index + 1]);
    if (provider !== null) return provider[1];
  }
  return undefined;
};

export const parseNmPerObject = (text) => {
  const entries = [];
  let currentObject;
  for (const line of text.split("\n")) {
    const header = /^(\S+\.o):$/.exec(line);
    if (header !== null) {
      currentObject = header[1];
      continue;
    }
    const symbol = /^[0-9a-f]*\s*([A-Za-z])\s+(\S+)$/.exec(line.trim());
    if (symbol !== null && currentObject !== undefined) {
      entries.push({ object: currentObject, type: symbol[1], name: symbol[2] });
    }
  }
  return entries;
};

const main = () => {
  const args = process.argv.slice(2);
  const option = (name) => {
    const index = args.indexOf(`--${name}`);
    return index >= 0 ? args[index + 1] : undefined;
  };
  const variant = option("variant");
  if (!["vulnerable", "fixed", "fault_build"].includes(variant ?? "")) {
    refuse("variant_invalid", String(variant));
  }
  const workdir = option("workdir") ?? "/tmp/coldcard-build";
  const outputPath = option("out") ?? join(workdir, "capture.json");
  const sourceName = variant === "vulnerable" ? "vulnerable" : "fixed";
  const sourceTree = join(SOURCE_ROOT, sourceName);
  if (!existsSync(sourceTree)) refuse("source_tree_absent", sourceTree);

  const pins = JSON.parse(readFileSync(join(SOURCE_ROOT, "pins.json"), "utf8"));
  const pin = pins[sourceName];
  const toolchainDigest = digestOfText(
    mustRun("arm-none-eabi-gcc", ["-v"], { stdio: ["ignore", "pipe", "pipe"] }) +
      run("arm-none-eabi-gcc", ["-v"]).stderr,
  );

  // The baked tree is root-owned and carries git's read-only pack directories,
  // so the working copy is made writable before the build touches it.
  const tree = join(workdir, "tree");
  mkdirSync(workdir, { recursive: true });
  mustRun("cp", ["-a", sourceTree, tree]);
  mustRun("chmod", ["-R", "u+w", tree]);

  let fault;
  if (variant === "fault_build") fault = applyFaultMutation(tree);

  const stm32 = join(tree, "stm32");
  const buildStartedAt = Date.now();
  const build = run("make", ["-f", "MK-Makefile", "DEBUG_BUILD=0", "V=1", "all"], {
    cwd: stm32,
    env: { ...process.env, LC_ALL: "C" },
  });
  const buildLog = `${build.stdout ?? ""}\n${build.stderr ?? ""}`;
  const exitStatus = build.status === null ? 128 : build.status;
  const buildMillis = Date.now() - buildStartedAt;

  const collected = join(workdir, "collected");
  mkdirSync(collected, { recursive: true });
  const buildDir = join(tree, BUILD_SUBDIR);

  const artifacts = [];
  const addAvailable = (kind, artifactRef, path) => {
    artifacts.push({ artifactRef, digest: digestOfFile(path), kind, status: "available" });
    return artifactRef;
  };
  const addMissing = (kind, artifactRef, reason) => {
    artifacts.push({ artifactRef, kind, status: "missing", unavailableReasonRef: reason });
    return undefined;
  };

  const buildLogPath = join(collected, "build.log");
  writeFileSync(buildLogPath, buildLog);
  const buildLogRef = addAvailable(
    "build_log",
    ref("artifact", variant, "build-log"),
    buildLogPath,
  );

  const commandsPath = join(collected, "compiler-commands.txt");
  writeFileSync(commandsPath, compilerCommandInventory(buildLog));
  const commandsRef = addAvailable(
    "compiler_command",
    ref("artifact", variant, "compiler-commands"),
    commandsPath,
  );

  // Objects: register exactly the RNG-chain objects individually, and every
  // object the link consumed as one archive, so a symbol-absence claim is
  // enumerated over the whole link rather than a curated subset.
  const objectPaths = walk(buildDir, (path) => path.endsWith(".o")).sort();
  const objectRefs = new Map();
  for (const suffix of WITNESSED_OBJECTS) {
    const path = join(buildDir, suffix);
    if (existsSync(path)) {
      objectRefs.set(suffix, addAvailable("object", ref("artifact", variant, "object", suffix), path));
    } else {
      addMissing("object", ref("artifact", variant, "object", suffix), "reason.coldcard.object_not_produced");
    }
  }

  let archiveRef;
  if (objectPaths.length > 0) {
    const archivePath = join(collected, "linked-objects.a");
    // A real ar archive of exactly the objects this build produced. It exists
    // so the symbol inventory has one artifact that covers the entire link.
    mustRun(
      "arm-none-eabi-ar",
      ["rcs", archivePath, ...objectPaths.map((path) => relative(buildDir, path))],
      { cwd: buildDir },
    );
    archiveRef = addAvailable("archive", ref("artifact", variant, "linked-objects"), archivePath);
  } else {
    archiveRef = addMissing(
      "archive",
      ref("artifact", variant, "linked-objects"),
      "reason.coldcard.no_objects_produced",
    );
  }

  const nmObjectsPath = join(collected, "nm-objects.txt");
  writeFileSync(
    nmObjectsPath,
    objectPaths.length === 0
      ? ""
      : objectPaths
          .map((path) => {
            const relativePath = relative(buildDir, path);
            const dump = run("arm-none-eabi-nm", ["--defined-only", relativePath], {
              cwd: buildDir,
            }).stdout;
            return `${relativePath}:\n${dump}`;
          })
          .join("\n"),
  );
  const nmObjectsRef = addAvailable(
    "symbol_table",
    ref("artifact", variant, "nm-objects"),
    nmObjectsPath,
  );

  const elfPath = join(buildDir, "firmware.elf");
  let nmFirmwareRef;
  if (existsSync(elfPath)) {
    const nmFirmwarePath = join(collected, "nm-firmware.txt");
    writeFileSync(nmFirmwarePath, run("arm-none-eabi-nm", ["--defined-only", elfPath]).stdout);
    nmFirmwareRef = addAvailable(
      "symbol_table",
      ref("artifact", variant, "nm-firmware"),
      nmFirmwarePath,
    );
  } else {
    nmFirmwareRef = addMissing(
      "symbol_table",
      ref("artifact", variant, "nm-firmware"),
      "reason.coldcard.link_did_not_produce_elf",
    );
  }

  const mapPath = join(buildDir, "firmware.map");
  const mapText = existsSync(mapPath) ? readFileSync(mapPath, "utf8") : undefined;
  const linkMapRef =
    mapText === undefined
      ? addMissing("link_map", ref("artifact", variant, "link-map"), "reason.coldcard.link_map_not_produced")
      : addAvailable("link_map", ref("artifact", variant, "link-map"), mapPath);

  const firmwarePath = join(buildDir, "firmware.dfu");
  const firmwareRef = existsSync(firmwarePath)
    ? addAvailable("firmware", ref("artifact", variant, "firmware"), firmwarePath)
    : addMissing("firmware", ref("artifact", variant, "firmware"), "reason.coldcard.firmware_not_produced");

  let debugRef;
  if (existsSync(elfPath)) {
    const debugPath = join(collected, "debug-info.bin");
    mustRun("arm-none-eabi-objcopy", [
      "--only-section=.debug_info",
      "-O",
      "binary",
      elfPath,
      debugPath,
    ]);
    debugRef = addAvailable("debug_metadata", ref("artifact", variant, "debug-info"), debugPath);
  } else {
    debugRef = addMissing(
      "debug_metadata",
      ref("artifact", variant, "debug-info"),
      "reason.coldcard.link_did_not_produce_elf",
    );
  }

  // Preprocessed source, replayed from the exact recorded compile command.
  const macroObservations = [];
  const preprocessedRefs = [];
  const boardCommand = compilerCommandFor(buildLog, `build-${BOARD}/boards/${BOARD}/rng.o`);
  if (boardCommand !== undefined) {
    const path = join(collected, "board-rng.i");
    if (preprocess(boardCommand, join(tree, "external/micropython/ports/stm32"), path, ["-dD"])) {
      preprocessedRefs.push(
        addAvailable("preprocessed_source", ref("artifact", variant, "preprocessed", "board-rng"), path),
      );
      const macroDumpPath = join(collected, "board-rng.macros");
      if (
        preprocess(boardCommand, join(tree, "external/micropython/ports/stm32"), macroDumpPath, [
          "-dM",
        ])
      ) {
        const guard = macroValue(readFileSync(macroDumpPath, "utf8"), "MICROPY_HW_ENABLE_RNG");
        if (guard !== undefined) {
          macroObservations.push({
            macroName: "macro.MICROPY_HW_ENABLE_RNG",
            preprocessedArtifactRef: preprocessedRefs[0],
            value: guard,
          });
        }
      }
    }
  }
  const upstreamCommand = compilerCommandFor(buildLog, `build-${BOARD}/rng.o`);
  if (upstreamCommand !== undefined && !upstreamCommand.includes("/dev/null")) {
    const path = join(collected, "upstream-rng.i");
    if (preprocess(upstreamCommand, join(tree, "external/micropython/ports/stm32"), path, ["-dD"])) {
      preprocessedRefs.push(
        addAvailable(
          "preprocessed_source",
          ref("artifact", variant, "preprocessed", "upstream-rng"),
          path,
        ),
      );
    }
  }
  if (preprocessedRefs.length === 0) {
    addMissing(
      "preprocessed_source",
      ref("artifact", variant, "preprocessed", "board-rng"),
      "reason.coldcard.compile_command_not_recorded",
    );
  }

  // Symbol inventory: every defined symbol of every object the link consumed,
  // attributed to the archive that contains them all.
  const symbolInventory = [];
  const symbolInventorySourceArtifactRefs = [];
  if (archiveRef !== undefined) {
    for (const entry of parseNmPerObject(readFileSync(nmObjectsPath, "utf8"))) {
      symbolInventory.push({
        definedInArtifactRef: archiveRef,
        providerRef: ref("provider.object", entry.object),
        symbolName: ref("symbol.defined", entry.name),
      });
    }
    symbolInventorySourceArtifactRefs.push(archiveRef);
  }
  for (const value of objectRefs.values()) symbolInventorySourceArtifactRefs.push(value);
  if (nmObjectsRef !== undefined) symbolInventorySourceArtifactRefs.push(nmObjectsRef);
  if (nmFirmwareRef !== undefined) symbolInventorySourceArtifactRefs.push(nmFirmwareRef);
  if (linkMapRef !== undefined) symbolInventorySourceArtifactRefs.push(linkMapRef);

  // Call graph over exactly the registered RNG-chain objects.
  const callEdges = [];
  const callGraphSourceArtifactRefs = [];
  let unresolvedIndirectCallSites = 0;
  const seenEdges = new Set();
  for (const [suffix, artifactRef] of objectRefs) {
    const dump = run("arm-none-eabi-objdump", ["-dr", suffix], { cwd: buildDir }).stdout ?? "";
    const { edges, unresolvedIndirect } = callSites(dump);
    unresolvedIndirectCallSites += unresolvedIndirect;
    callGraphSourceArtifactRefs.push(artifactRef);
    for (const edge of edges) {
      const key = `${edge.from}->${edge.to}`;
      if (seenEdges.has(key)) continue;
      seenEdges.add(key);
      const evidence = [artifactRef];
      if (linkMapRef !== undefined) evidence.push(linkMapRef);
      if (debugRef !== undefined) evidence.push(debugRef);
      callEdges.push({
        evidenceArtifactRefs: evidence,
        fromSymbol: ref("symbol.linked", edge.from),
        toSymbol: ref("symbol.linked", edge.to),
      });
    }
  }
  if (linkMapRef !== undefined) callGraphSourceArtifactRefs.push(linkMapRef);

  // Which object the linker actually chose for rng_get.
  const symbolProviders = [];
  const linkedProvider = mapText === undefined ? undefined : linkedProviderOf(mapText, "rng_get");
  if (
    linkedProvider !== undefined &&
    archiveRef !== undefined &&
    nmObjectsRef !== undefined &&
    linkMapRef !== undefined
  ) {
    // Longest suffix wins: `boards/COLDCARD_MK4/rng.o` also ends with `rng.o`,
    // and picking the shorter one would attribute the fixed build's approved
    // provider to the upstream fallback object.
    const providerSuffix = [...objectRefs.keys()]
      .sort((left, right) => right.length - left.length)
      .find((suffix) => linkedProvider.endsWith(`/${suffix}`) || linkedProvider === suffix);
    if (providerSuffix !== undefined) {
      symbolProviders.push({
        archiveArtifactRef: archiveRef,
        linkMapArtifactRef: linkMapRef,
        objectArtifactRef: objectRefs.get(providerSuffix),
        providerRef: ref("provider.object", providerSuffix),
        symbolName: "symbol.linked.rng_get",
        symbolTableArtifactRef: nmObjectsRef,
      });
    }
  }

  const sourceBundleDigest = digestOfText(
    mustRun("git", ["rev-parse", "HEAD"], { cwd: tree }) +
      mustRun("git", ["submodule", "status"], { cwd: tree }) +
      (fault === undefined ? "" : `${fault.beforeDigest}->${fault.afterDigest}`),
  );

  const buildConfigurationDigest = digestOfText(
    JSON.stringify({
      board: BOARD,
      debugBuild: 0,
      driverRef: DRIVER_REF,
      makefile: "MK-Makefile",
      target: "all",
      verbose: 1,
    }),
  );

  const capture = {
    schema: CAPTURE_SCHEMA,
    artifacts,
    buildConfigurationDigest,
    buildTermination:
      buildLogRef === undefined
        ? { status: "unobserved", unavailableReasonRef: "reason.coldcard.build_log_not_collected" }
        : { buildLogArtifactRef: buildLogRef, exitStatus, status: "observed" },
    callEdges,
    callGraphSourceArtifactRefs,
    captureRef: ref("capture.coldcard", variant),
    capturedAt: new Date().toISOString(),
    ...(fault === undefined ? {} : { faultMutationRef: fault.faultMutationRef }),
    macroObservations,
    // Provenance is deliberately left for the operator harness: a guest cannot
    // attest to its own admission. The harness replaces this placeholder with
    // the live managed-sandbox receipts or the capture is refused.
    provenance: {
      conformanceNoteRef: "note.artifact_witness.guest_capture_awaiting_operator_provenance",
      kind: "conformance_vector",
    },
    sourceBundleDigest,
    symbolInventory,
    symbolInventorySourceArtifactRefs,
    symbolProviders,
    targetCommit: pin.commitSha,
    targetSnapshotRef: ref("target.coldcard", variant),
    toolchainDigest,
    unresolvedIndirectCallSites,
    variant,
    widthObservations: [],
    workerPlacementRef: process.env.OA_COLDCARD_PLACEMENT_REF ?? "placement.coldcard.pending",
    workerProfileDigest:
      process.env.OA_COLDCARD_WORKER_PROFILE_DIGEST ??
      "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  };

  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, JSON.stringify(capture));
  process.stdout.write(
    `${JSON.stringify({
      driverRef: DRIVER_REF,
      variant,
      exitStatus,
      buildMillis,
      capturePath: outputPath,
      captureDigest: digestOfFile(outputPath),
      captureBytes: statSync(outputPath).size,
      artifactCount: artifacts.length,
      availableArtifactKinds: [
        ...new Set(
          artifacts.filter((entry) => entry.status === "available").map((entry) => entry.kind),
        ),
      ].sort(),
      symbolInventoryEntries: symbolInventory.length,
      callEdges: callEdges.length,
      unresolvedIndirectCallSites,
      linkedRngProvider: linkedProvider ?? null,
      macroObservations,
    })}\n`,
  );
};

// Run only when executed directly, so the pure helpers above stay importable
// by tests. Comparing resolved URLs keeps this correct under any install name.
if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main();
}
