#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { promises as dns } from "node:dns";
import { lstatSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { connect } from "node:tls";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { validateDeploymentBundle } from "./livekit-ops-policy.mjs";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";
const COMMIT = /^[0-9a-f]{40}$/u;
const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const SIGNAL_HOST = "livekit.openagents.com";
const TURN_HOST = "turn.livekit.openagents.com";

const usage = () => {
  process.stderr.write(`Usage:
  OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \\
  node scripts/cloud/livekit-connectivity-inventory.mjs \\
    --bundle infra/livekit/bundle.json \\
    --deployed-revision <40-hex> \\
    --packaged-omega-attestation <private-json> \\
    --output <private-inventory.json> \\
    [--context <kubectl-context>] --apply

Performs read-only Kubernetes, DNS, and TLS observations and writes an
exclusive mode-0600 connectivity inventory. It never reads Kubernetes Secrets,
logs, media, transcripts, bearer tokens, or provider credentials.
`);
};

const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const exactKeys = (value, keys, label) => {
  assert(value && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  assert(
    JSON.stringify(Object.keys(value).toSorted()) === JSON.stringify([...keys].toSorted()),
    `${label} fields do not match the closed schema`,
  );
};

const isWithin = (parent, candidate) => {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
};

const readJson = (path, label) => {
  try {
    return JSON.parse(readFileSync(resolve(path), "utf8"));
  } catch (error) {
    throw new Error(`${label} is not readable JSON`, { cause: error });
  }
};

const assertPrivateInput = (path) => {
  const input = lstatSync(resolve(path));
  assert(
    input.isFile() && !input.isSymbolicLink() && (input.mode & 0o077) === 0,
    "packaged Omega attestation must be a mode-0600-or-stricter regular file",
  );
};

const parseArgs = (args) => {
  const parsed = { apply: false };
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    if (
      ![
        "--bundle",
        "--context",
        "--deployed-revision",
        "--output",
        "--packaged-omega-attestation",
      ].includes(argument)
    ) {
      throw new Error(`unsupported argument ${argument}`);
    }
    const value = args[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    parsed[
      {
        "--bundle": "bundle",
        "--context": "context",
        "--deployed-revision": "deployedRevision",
        "--output": "output",
        "--packaged-omega-attestation": "packagedOmegaAttestation",
      }[argument]
    ] = value;
    index += 1;
  }
  for (const key of ["bundle", "deployedRevision", "output", "packagedOmegaAttestation"]) {
    if (!parsed[key]) throw new Error(`missing required argument ${key}`);
  }
  if (!parsed.apply) throw new Error("live inventory collection requires --apply");
  if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`);
  }
  if (!COMMIT.test(parsed.deployedRevision)) {
    throw new Error("deployed revision must be a full Git commit");
  }
  return parsed;
};

const kubectlJson = (resourceArgs, context) => {
  const args = [
    ...(context ? ["--context", context] : []),
    "--namespace",
    "livekit-system",
    "get",
    ...resourceArgs,
    "--output",
    "json",
  ];
  const result = spawnSync("kubectl", args, {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    shell: false,
    timeout: 60_000,
  });
  if (result.error) {
    throw new Error("kubectl inventory query could not execute", { cause: result.error });
  }
  if (result.status !== 0) throw new Error("kubectl inventory query failed");
  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    throw new Error("kubectl inventory query returned invalid JSON", { cause: error });
  }
};

const addresses = (value) =>
  (value?.status?.loadBalancer?.ingress ?? [])
    .flatMap((entry) => [entry.ip, entry.hostname])
    .filter((entry) => typeof entry === "string" && entry.length > 0);

const resolveAddresses = async (hostname) => {
  const results = await Promise.allSettled([dns.resolve4(hostname), dns.resolve6(hostname)]);
  return new Set(results.flatMap((result) => (result.status === "fulfilled" ? result.value : [])));
};

const tlsAuthorized = (hostname) =>
  new Promise((resolvePromise) => {
    const socket = connect({ host: hostname, port: 443, servername: hostname, timeout: 15_000 });
    const finish = (result) => {
      socket.destroy();
      resolvePromise(result);
    };
    socket.once("secureConnect", () => {
      const certificate = socket.getPeerCertificate();
      finish({
        authorized: socket.authorized,
        unexpired:
          typeof certificate.valid_to === "string" &&
          Number.isFinite(Date.parse(certificate.valid_to)) &&
          Date.parse(certificate.valid_to) > Date.now(),
      });
    });
    socket.once("timeout", () => finish({ authorized: false, unexpired: false }));
    socket.once("error", () => finish({ authorized: false, unexpired: false }));
  });

const readyCondition = (certificate) =>
  certificate?.status?.conditions?.some(
    (condition) => condition.type === "Ready" && condition.status === "True",
  ) === true;

const dnsMatches = (expected, resolved) =>
  expected.length > 0 && expected.every((address) => resolved.has(address));

export const projectConnectivityInventory = ({
  bundle,
  deployedRevision,
  packagedOmega,
  serverDeployment,
  workerDeployment,
  serverConfig,
  nodes,
  signalingIngress,
  turnService,
  signalingCertificate,
  turnCertificate,
  signalingDns,
  turnDns,
  signalingTls,
  turnTls,
  observedAt = new Date().toISOString(),
}) => {
  exactKeys(
    packagedOmega,
    ["schemaVersion", "observedAt", "releaseSigned", "launchSucceeded", "artifactDigest"],
    "packaged Omega attestation",
  );
  assert(
    packagedOmega.schemaVersion === "openagents.omega_packaged_attestation.v1" &&
      Number.isFinite(Date.parse(packagedOmega.observedAt)) &&
      Date.parse(packagedOmega.observedAt) <= Date.parse(observedAt) + 5 * 60_000 &&
      Date.parse(observedAt) - Date.parse(packagedOmega.observedAt) <= 24 * 60 * 60_000 &&
      packagedOmega.releaseSigned === true &&
      packagedOmega.launchSucceeded === true &&
      DIGEST.test(packagedOmega.artifactDigest),
    "packaged Omega attestation is not a valid signed, launched artifact observation",
  );
  const serverContainers = serverDeployment?.spec?.template?.spec?.containers ?? [];
  const workerContainers = workerDeployment?.spec?.template?.spec?.containers ?? [];
  const digestPinnedImages = [...serverContainers, ...workerContainers].every(
    (container) =>
      typeof container.image === "string" && /@sha256:[0-9a-f]{64}$/u.test(container.image),
  );
  const sfuNodeExternalAddressCount = (nodes?.items ?? []).filter((node) =>
    (node?.status?.addresses ?? []).some(
      (address) => address.type === "ExternalIP" && typeof address.address === "string",
    ),
  ).length;
  const signalAddresses = addresses(signalingIngress);
  const turnAddresses = addresses(turnService);
  const signalingManagedCertificateActive =
    signalingCertificate?.status?.certificateStatus === "Active";
  const turnCertificateReady = readyCondition(turnCertificate);
  return {
    schemaVersion: "openagents.livekit_connectivity_inventory.v1",
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision,
    observedAt,
    packagedOmega: {
      releaseSigned: packagedOmega.releaseSigned,
      launchSucceeded: packagedOmega.launchSucceeded,
      artifactDigest: packagedOmega.artifactDigest,
    },
    runtime: {
      serverDesiredReplicas: serverDeployment?.spec?.replicas ?? 0,
      serverReadyReplicas: serverDeployment?.status?.readyReplicas ?? 0,
      workerDesiredReplicas: workerDeployment?.spec?.replicas ?? 0,
      workerReadyReplicas: workerDeployment?.status?.readyReplicas ?? 0,
      digestPinnedImages,
      hostNetwork: serverDeployment?.spec?.template?.spec?.hostNetwork === true,
      externalIpDiscoveryEnabled: /^\s*use_external_ip:\s*true\s*$/mu.test(
        serverConfig?.data?.["config.yaml"] ?? "",
      ),
      sfuNodeExternalAddressCount,
    },
    network: {
      signalingAddressProvisioned: signalAddresses.length > 0,
      turnAddressProvisioned: turnAddresses.length > 0,
      signalingDnsMatchesAddress: dnsMatches(signalAddresses, signalingDns),
      turnDnsMatchesAddress: dnsMatches(turnAddresses, turnDns),
      signalingTlsAuthorized: signalingManagedCertificateActive && signalingTls.authorized === true,
      turnTlsAuthorized: turnCertificateReady && turnTls.authorized === true,
      signalingCertificateUnexpired:
        signalingManagedCertificateActive && signalingTls.unexpired === true,
      turnCertificateUnexpired: turnCertificateReady && turnTls.unexpired === true,
    },
  };
};

const run = async () => {
  const args = parseArgs(process.argv.slice(2));
  const bundle = validateDeploymentBundle(readJson(args.bundle, "deployment bundle"));
  assertPrivateInput(args.packagedOmegaAttestation);
  const packagedOmega = readJson(args.packagedOmegaAttestation, "packaged Omega attestation");
  const [
    serverDeployment,
    workerDeployment,
    serverConfig,
    nodes,
    signalingIngress,
    turnService,
    signalingCertificate,
    turnCertificate,
    signalingDns,
    turnDns,
    signalingTls,
    turnTls,
  ] = await Promise.all([
    Promise.resolve(kubectlJson(["deployment", "livekit-server"], args.context)),
    Promise.resolve(kubectlJson(["deployment", "sarah-livekit-agent"], args.context)),
    Promise.resolve(kubectlJson(["configmap", "livekit-server"], args.context)),
    Promise.resolve(
      kubectlJson(["nodes", "--selector", "openagents.com/livekit-workload=sfu"], args.context),
    ),
    Promise.resolve(kubectlJson(["ingress", "livekit-server"], args.context)),
    Promise.resolve(kubectlJson(["service", "livekit-server-turn"], args.context)),
    Promise.resolve(kubectlJson(["managedcertificate", "livekit-signal"], args.context)),
    Promise.resolve(kubectlJson(["certificate", "livekit-turn"], args.context)),
    resolveAddresses(SIGNAL_HOST),
    resolveAddresses(TURN_HOST),
    tlsAuthorized(SIGNAL_HOST),
    tlsAuthorized(TURN_HOST),
  ]);
  const inventory = projectConnectivityInventory({
    bundle,
    deployedRevision: args.deployedRevision,
    packagedOmega,
    serverDeployment,
    workerDeployment,
    serverConfig,
    nodes,
    signalingIngress,
    turnService,
    signalingCertificate,
    turnCertificate,
    signalingDns,
    turnDns,
    signalingTls,
    turnTls,
  });
  const output = resolve(args.output);
  const repositoryRoot = resolve(import.meta.dirname, "../..");
  assert(!isWithin(repositoryRoot, output), "private inventory must stay outside the repository");
  mkdirSync(dirname(output), { recursive: true, mode: 0o700 });
  writeFileSync(output, `${JSON.stringify(inventory, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({
      outcome: "collected",
      output,
      runtimeReady:
        inventory.runtime.serverReadyReplicas === inventory.runtime.serverDesiredReplicas &&
        inventory.runtime.workerReadyReplicas === inventory.runtime.workerDesiredReplicas,
      signalingDnsMatchesAddress: inventory.network.signalingDnsMatchesAddress,
      turnDnsMatchesAddress: inventory.network.turnDnsMatchesAddress,
    })}\n`,
  );
};

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  run().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    usage();
    process.exitCode = 1;
  });
}
