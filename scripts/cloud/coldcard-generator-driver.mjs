#!/usr/bin/env node

/**
 * Guest-side Coldcard generator capture driver (openagents #9297, OFR-015).
 *
 * Runs INSIDE an admitted OpenAgents Cloud managed sandbox. It compiles the
 * pinned libngu `ngu/random.c` from the read-only Coldcard tree baked into the
 * guest image, drives that compiled target source to produce the expected
 * values for all eight frozen generator vectors, and measures a candidate
 * search rate through the same code.
 *
 * The driver reports observations only. It does not decide whether the run is
 * admissible, and it does not write provenance: a guest cannot attest to its
 * own admission. The host harness stamps provenance from control-plane
 * receipts it observed, exactly as the OFR-014 artifact-witness run does.
 */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

const CAPTURE_SCHEMA = "openagents.coldcard_generator_capture.v1";
const SYMBOLS = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

const MASK32 = 0xffffffffn;
const uint32 = (value) => Number(BigInt(value) & MASK32);

/**
 * The eight vector configurations, one per fixture class.
 *
 * These are the same initial states the frozen corpus has always used. They
 * are inputs, not evidence: nothing about a configuration is asserted here.
 * What changes with this run is where the EXPECTED values come from.
 */
const VECTOR_CONFIGS = [
  {
    fixtureClass: "vulnerable",
    libngu: [0x0a8ce26f, 69, 233, 0],
    provider: { kind: "yasmarang", state: [0x11223344, 0x01020304, 0x05060708, 0] },
  },
  {
    fixtureClass: "partially_mitigated",
    libngu: [0x0a8ce26f, 69, 233, 0],
    provider: { kind: "yasmarang", state: [0x11223344, 0x01020304, 0x05060708, 0] },
    reseed: "0x1122334455667788",
  },
  {
    fixtureClass: "fixed",
    libngu: [0x0a8ce26f, 69, 233, 0],
    provider: {
      kind: "approved_fixture",
      words: Array.from({ length: 64 }, (_, index) => uint32(0x9e3779b9n * BigInt(index + 1))),
    },
  },
  {
    fixtureClass: "mutated_guard",
    libngu: [0x0a8ce26f, 69, 233, 0],
    provider: {
      kind: "approved_fixture",
      words: Array.from({ length: 64 }, (_, index) => (0x6a09e667 ^ (index * 0x01010101)) >>> 0),
    },
  },
  {
    fixtureClass: "mutated_provider",
    libngu: [0x0a8ce26f, 69, 233, 0],
    provider: { kind: "yasmarang", state: [0xdeadbeef, 69, 233, 0] },
  },
  {
    fixtureClass: "mutated_initialization",
    libngu: [0x0a8ce26f, 69, 233, 1],
    provider: { kind: "yasmarang", state: [0x11223345, 0x01020304, 0x05060708, 1] },
  },
  {
    // The state a vulnerable run reaches after four bytes. Derived below from a
    // prestep invocation of the same compiled target source, never written in.
    fixtureClass: "mutated_call_trace",
    derivedFrom: { fixtureClass: "vulnerable", outputLength: 4 },
  },
  {
    fixtureClass: "mutated_reseed_truncation",
    libngu: [(0x55667788 ^ 0x11223344) >>> 0, 69, 233, 0],
    provider: { kind: "yasmarang", state: [0x11223344, 0x01020304, 0x05060708, 0] },
  },
];

const sha256Hex = (value) => createHash("sha256").update(value).digest("hex");

function parseArgs() {
  const args = process.argv.slice(2);
  const option = (name, fallback) => {
    const index = args.indexOf(`--${name}`);
    return index >= 0 && args[index + 1] !== undefined ? args[index + 1] : fallback;
  };
  return {
    randomC: option("random-c", "/opt/coldcard/vulnerable/external/libngu/ngu/random.c"),
    sourceDir: option("source-dir", join(dirname(resolve(process.argv[1])), "generator-guest")),
    workdir: option("workdir", "/workspace/scratch/generator"),
    out: option("out", "/workspace/scratch/generator-capture.json"),
    minNanos: option("min-nanos", "10000000000"),
    batch: option("batch", "8192"),
    cc: option("cc", ["/usr/bin/cc", "/usr/bin/gcc"].find((path) => existsSync(path)) ?? ""),
    pins: option("pins", "/opt/coldcard/pins.json"),
  };
}

const { randomC, sourceDir, workdir, out, minNanos, batch, cc, pins } = parseArgs();
if (cc === "") throw new Error("no host C compiler found at /usr/bin/cc or /usr/bin/gcc");

mkdirSync(workdir, { recursive: true });

const randomCBytes = readFileSync(randomC);
const sourceDigest = `sha256:${sha256Hex(randomCBytes)}`;

const compilerVersion = execFileSync(cc, ["--version"], { encoding: "utf8" }).split("\n")[0].trim();

const baseFlags = [
  "-O2",
  "-std=c11",
  // clock_gettime and CLOCK_MONOTONIC are POSIX, and strict ISO C hides them
  // behind this feature test macro on glibc.
  "-D_POSIX_C_SOURCE=200809L",
  `-I${join(sourceDir, "shim")}`,
  `-I${sourceDir}`,
  `-DOA_LIBNGU_RANDOM_C="${randomC}"`,
];
// The provider instance is a second verbatim compilation of the same pinned
// source. Its four external symbols are renamed so both objects can link.
const providerFlags = [
  "-Dmy_random_bytes=oa_prov_my_random_bytes",
  "-D_bit_length=oa_prov_bit_length",
  "-D_rand_below=oa_prov_rand_below",
  "-Dmp_module_random=oa_prov_mp_module_random",
];

const units = [
  { source: "oa_libngu.c", object: "oa_libngu.o", extra: [] },
  { source: "oa_provider.c", object: "oa_provider.o", extra: providerFlags },
  { source: "oa_shim.c", object: "oa_shim.o", extra: [] },
  { source: "oa_main.c", object: "oa_main.o", extra: [] },
];

const compileCommands = [];
for (const unit of units) {
  const argv = [
    ...baseFlags,
    ...unit.extra,
    "-c",
    join(sourceDir, unit.source),
    "-o",
    join(workdir, unit.object),
  ];
  execFileSync(cc, argv, { stdio: ["ignore", "pipe", "pipe"] });
  compileCommands.push({ argv: [cc, ...argv], sourceRef: unit.source });
}
const binary = join(workdir, "oa-coldcard-generator");
const linkArgv = [...units.map((unit) => join(workdir, unit.object)), "-o", binary];
execFileSync(cc, linkArgv, { stdio: ["ignore", "pipe", "pipe"] });

const harnessSourceDigests = Object.fromEntries([
  // This driver chose the vector configurations, so it is part of what the
  // capture has to be bound to. A test refuses a capture whose recorded
  // harness digests are not the files checked into the repository.
  ["coldcard-generator-driver.mjs", `sha256:${sha256Hex(readFileSync(resolve(process.argv[1])))}`],
  ...[
    "oa_libngu.c",
    "oa_provider.c",
    "oa_shim.c",
    "oa_main.c",
    "oa_harness.h",
    "shim/py/runtime.h",
    "shim/py/mperrno.h",
  ].map((relative) => [relative, `sha256:${sha256Hex(readFileSync(join(sourceDir, relative)))}`]),
]);

const runVector = (config, outputLength) => {
  const argv = ["vector", "--libngu", config.libngu.join(","), "--count", String(outputLength)];
  if (config.provider.kind === "yasmarang") {
    argv.push("--provider-yasmarang", config.provider.state.join(","));
  } else {
    argv.push("--provider-fixture", config.provider.words.join(","));
  }
  if (config.reseed !== undefined) argv.push("--reseed", config.reseed);
  argv.push("--symbols", SYMBOLS.join(","));
  const stdout = execFileSync(binary, argv, {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  });
  return { argv, capture: JSON.parse(stdout) };
};

const resolved = new Map();
const vectors = [];
for (const config of VECTOR_CONFIGS) {
  let effective = config;
  let derivedFromCapture;
  if (config.derivedFrom !== undefined) {
    const base = resolved.get(config.derivedFrom.fixtureClass);
    if (base === undefined)
      throw new Error(`prestep base ${config.derivedFrom.fixtureClass} first`);
    const prestep = runVector(base, config.derivedFrom.outputLength);
    const state = prestep.capture.postBytesState;
    if (state.provider.kind !== "yasmarang") throw new Error("prestep provider must be yasmarang");
    effective = {
      fixtureClass: config.fixtureClass,
      libngu: [state.libngu.pad, state.libngu.n, state.libngu.d, state.libngu.dat],
      provider: {
        kind: "yasmarang",
        state: [
          state.provider.state.pad,
          state.provider.state.n,
          state.provider.state.d,
          state.provider.state.dat,
        ],
      },
    };
    derivedFromCapture = { argv: prestep.argv, postBytesState: state, ...config.derivedFrom };
  }
  resolved.set(config.fixtureClass, effective);
  const run = runVector(effective, 32);
  vectors.push({
    fixtureClass: config.fixtureClass,
    outputLength: 32,
    argv: run.argv,
    ...(derivedFromCapture === undefined ? {} : { derivedFrom: derivedFromCapture }),
    ...(effective.provider.kind === "approved_fixture"
      ? { providerWords: effective.provider.words }
      : {}),
    ...(config.reseed === undefined ? {} : { reseed: config.reseed }),
    capture: run.capture,
  });
}

const throughputArgv = ["throughput", "--min-nanos", minNanos, "--batch", batch];
const throughput = JSON.parse(
  execFileSync(binary, throughputArgv, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }),
);

const capture = {
  schema: CAPTURE_SCHEMA,
  compiler: { command: cc, version: compilerVersion, compileCommands, linkArgv: [cc, ...linkArgv] },
  harnessSourceDigests,
  observedAt: new Date().toISOString(),
  targetSource: {
    path: randomC,
    sourceDigest,
    byteLength: randomCBytes.length,
    pins: JSON.parse(readFileSync(pins, "utf8")),
  },
  throughput: { argv: throughputArgv, ...throughput },
  vectors,
  workerProfileDigest: process.env.OA_COLDCARD_WORKER_PROFILE_DIGEST ?? "",
};

writeFileSync(out, JSON.stringify(capture), { mode: 0o600 });
process.stdout.write(
  `${JSON.stringify({
    out,
    sourceDigest,
    compilerVersion,
    vectorCount: vectors.length,
    candidatesEvaluated: throughput.candidatesEvaluated,
    elapsedNanoseconds: throughput.elapsedNanoseconds,
  })}\n`,
);
