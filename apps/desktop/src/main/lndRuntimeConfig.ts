import fs from "node:fs";
import path from "node:path";

export type LndNeutrinoNetwork = "mainnet" | "testnet" | "signet" | "regtest";

export type LndRuntimePaths = Readonly<{
  readonly rootDir: string;
  readonly runtimeDir: string;
  readonly dataDir: string;
  readonly logDir: string;
  readonly configDir: string;
  readonly configPath: string;
  readonly tlsCertPath: string;
  readonly tlsKeyPath: string;
}>;

export type LndRuntimeConfigInput = Readonly<{
  readonly userDataPath: string;
  readonly network: LndNeutrinoNetwork;
  readonly alias: string;
  readonly rpcListen: string;
  readonly restListen: string;
  readonly p2pListen: string;
  readonly debugLevel: string;
  readonly neutrinoPeers: ReadonlyArray<string>;
}>;

export type LndRuntimeConfigMaterialized = Readonly<{
  readonly paths: LndRuntimePaths;
  readonly configText: string;
  readonly launchArgs: ReadonlyArray<string>;
}>;

const normalizePath = (value: string): string => path.resolve(value);

export const resolveLndRuntimePaths = (input: {
  readonly userDataPath: string;
  readonly network: LndNeutrinoNetwork;
}): LndRuntimePaths => {
  const rootDir = normalizePath(path.join(input.userDataPath, "lnd"));
  const runtimeDir = path.join(rootDir, input.network);
  const dataDir = path.join(runtimeDir, "data");
  const logDir = path.join(runtimeDir, "logs");
  const configDir = path.join(runtimeDir, "config");
  const configPath = path.join(configDir, "lnd.conf");
  const tlsCertPath = path.join(configDir, "tls.cert");
  const tlsKeyPath = path.join(configDir, "tls.key");

  return {
    rootDir,
    runtimeDir,
    dataDir,
    logDir,
    configDir,
    configPath,
    tlsCertPath,
    tlsKeyPath,
  };
};

const normalizeAlias = (alias: string): string => {
  const trimmed = alias.trim();
  if (trimmed.length === 0) return "openagents-desktop";
  return trimmed.slice(0, 32);
};

const confLine = (key: string, value: string | number | boolean): string => `${key}=${String(value)}`;

export const buildLndConfText = (input: LndRuntimeConfigInput, paths: LndRuntimePaths): string => {
  const sections: Array<string> = [];

  sections.push("[Application Options]");
  sections.push(confLine("alias", normalizeAlias(input.alias)));
  sections.push(confLine("lnddir", paths.runtimeDir));
  sections.push(confLine("datadir", paths.dataDir));
  sections.push(confLine("logdir", paths.logDir));
  sections.push(confLine("tlscertpath", paths.tlsCertPath));
  sections.push(confLine("tlskeypath", paths.tlsKeyPath));
  sections.push(confLine("rpclisten", input.rpcListen));
  sections.push(confLine("restlisten", input.restListen));
  sections.push(confLine("listen", input.p2pListen));
  sections.push(confLine("debuglevel", input.debugLevel));
  sections.push("");

  sections.push("[Bitcoin]");
  sections.push(confLine("bitcoin.active", true));
  sections.push(confLine(`bitcoin.${input.network}`, true));
  sections.push(confLine("bitcoin.node", "neutrino"));
  sections.push("");

  sections.push("[Neutrino]");
  const peers = [...input.neutrinoPeers].sort((a, b) => a.localeCompare(b));
  for (const peer of peers) {
    sections.push(confLine("neutrino.addpeer", peer));
  }

  return `${sections.join("\n")}\n`;
};

export const buildLndLaunchArgs = (input: {
  readonly configPath: string;
}): ReadonlyArray<string> =>
  [
    // Network + neutrino mode are configured in lnd.conf.
    // Keep CLI args minimal to avoid boolean flag parser differences across lnd versions.
    `--configfile=${input.configPath}`,
  ] as const;

const ensureRuntimeDirectories = (paths: LndRuntimePaths): void => {
  fs.mkdirSync(paths.runtimeDir, { recursive: true });
  fs.mkdirSync(paths.dataDir, { recursive: true });
  fs.mkdirSync(paths.logDir, { recursive: true });
  fs.mkdirSync(paths.configDir, { recursive: true });
};

export const materializeLndRuntimeConfig = (input: LndRuntimeConfigInput): LndRuntimeConfigMaterialized => {
  const paths = resolveLndRuntimePaths({
    userDataPath: input.userDataPath,
    network: input.network,
  });

  ensureRuntimeDirectories(paths);

  const configText = buildLndConfText(input, paths);
  fs.writeFileSync(paths.configPath, configText, "utf8");

  const launchArgs = buildLndLaunchArgs({
    configPath: paths.configPath,
  });

  return {
    paths,
    configText,
    launchArgs,
  };
};
