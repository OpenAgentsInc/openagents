#!/usr/bin/env node
import chalk from "chalk";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import fs from "node:fs";
import path from "node:path";

const execFileP = promisify(execFile);

// Candidate CLI locations. Desktop apps bundle the CLI but don't add it to PATH.
const TAILSCALE_PATHS = [
  "tailscale", // hope it's on PATH (Linux/CLI installs)
  "/Applications/Tailscale.app/Contents/MacOS/tailscale", // macOS GUI bundle
  "C\\\\Program Files (x86)\\\\Tailscale IPN\\\\tailscale.exe", // Windows
];

// Private RFC1918 + ULA prefixes
const PRIVATE = /^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[0-1])\.)|^fd|^fc/i;
const cleanAddr = (addr?: string): string => {
  if (!addr) return "";
  // remove brackets and ports for IPv6/IPv4: "[fd00::1]:1234" or "192.168.1.2:41641"
  const noBracket = addr.replace(/^\[/, "").replace(/\]$/, "");
  return noBracket.replace(/:[0-9]+$/, "");
};

async function findTailscale(): Promise<string | null> {
  for (const p of TAILSCALE_PATHS) {
    try {
      const { stdout } = await execFileP(p, ["version"], { windowsHide: true });
      if (stdout.toLowerCase().includes("tailscale")) return p;
    } catch {
      // ignore and continue
    }
  }
  return null;
}

async function main() {
  // Detect CLI
  const tsPath = await findTailscale();
  if (!tsPath) {
    console.error(chalk.red("Tailscale CLI not found."));
    const plat = process.platform;
    if (plat === "darwin") {
      console.error(
        chalk.yellow(
          "Install via Homebrew: brew install tailscale (or open https://tailscale.com/download)"
        )
      );
    } else if (plat === "win32") {
      console.error(
        chalk.yellow("Install via winget: winget install Tailscale.Tailscale")
      );
    } else {
      console.error(
        chalk.yellow(
          "Install via script: curl -fsSL https://tailscale.com/install.sh | sh"
        )
      );
    }
    process.exitCode = 1;
    return;
  }

  console.log(chalk.green(`Found Tailscale CLI: ${tsPath}`));

  // Smoke test: try status --json, fall back to version
  try {
    const { stdout } = await execFileP(tsPath, ["status", "--json"], {
      windowsHide: true,
      maxBuffer: 5 * 1024 * 1024,
    });
    const data = JSON.parse(stdout || "{}") as any;
    const peers: any[] = Object.values(data?.Peer ?? {});
    const RAW = process.env.TRICODER_TAILSCALE_RAW === "1" || process.argv.includes("--raw");
    console.log(chalk.bold(`tailscale status: ${peers.length} peer${peers.length === 1 ? "" : "s"}`));

    // Show current device IPs first so the user can copy a reachable address
    const self = data?.Self || data?.SelfNode || data?.SelfStatus || null;
    const selfName = self?.DNSName || self?.HostName || self?.Hostname || "this-device";
    const selfIPs: string[] = Array.isArray(self?.TailscaleIPs) ? self.TailscaleIPs : [];
    const selfIPv4 = selfIPs.find((ip: string) => ip.includes("."));
    const selfIPv6 = selfIPs.find((ip: string) => ip.includes(":"));
    const bridgePort = (() => {
      const argPortIdx = process.argv.findIndex((a) => a === "--port");
      if (argPortIdx !== -1 && process.argv[argPortIdx + 1]) return Number(process.argv[argPortIdx + 1]);
      if (process.env.TRICODER_BRIDGE_PORT) return Number(process.env.TRICODER_BRIDGE_PORT);
      return 8787; // default oa-bridge bind
    })();
    const bracket = (ip?: string) => (ip && ip.includes(":") ? `[${ip}]` : ip || "");
    if (selfIPs.length > 0 || self) {
      console.log("\n" + chalk.magenta.bold(`${selfName} (this device)`));
      if (selfIPs.length > 0) {
        console.log("  " + chalk.gray("ts-ips:") + " " + selfIPs.join(", "));
      }
      const selfLanCandidates: string[] = Array.isArray(self?.Addrs)
        ? (self.Addrs as string[]).map(cleanAddr).filter((a) => a && PRIVATE.test(a))
        : [];
      if (selfLanCandidates.length > 0) {
        console.log("  " + chalk.gray("lan-ips:") + " " + selfLanCandidates.join(", "));
      }
      const chosen = selfIPv4 || selfIPv6;
      if (chosen) {
        const wsUrl = `ws://${bracket(chosen)}:${bridgePort}/ws`;
        const host = `${bracket(chosen)}:${bridgePort}`;
        console.log("  " + chalk.gray("bridge-host:"), host);
        console.log("  " + chalk.gray("bridge-ws:"), wsUrl);
        // If a bridge token is present, print a fully-qualified URL
        try {
          const home = process.env.OPENAGENTS_HOME || process.env.HOME;
          if (home) {
            const p = path.join(home, ".openagents", "bridge.json");
            if (fs.existsSync(p)) {
              const raw = JSON.parse(fs.readFileSync(p, "utf8"));
              const token = String(raw?.token || "").trim();
              if (token) {
                console.log("  " + chalk.gray("bridge-ws+token:"), `${wsUrl}?token=${encodeURIComponent(token)}`);
              }
            }
          }
        } catch {}
      }
      if (RAW && self) {
        console.log(chalk.gray("  raw:"));
        console.log(chalk.gray("  " + JSON.stringify(self, null, 2).split("\n").join("\n  ")));
      }
    }

    

    const fmt = (label: string, value: any) =>
      value === undefined || value === null || value === "" || (Array.isArray(value) && value.length === 0)
        ? null
        : `${chalk.gray(label)} ${Array.isArray(value) ? value.join(", ") : String(value)}`;

    for (const p of peers) {
      const name = p?.DNSName || p?.HostName || p?.Hostname || p?.Name || "(unknown)";
      const tsIPs: string[] = Array.isArray(p?.TailscaleIPs) ? p.TailscaleIPs : [];
      const endpoints: string[] = Array.isArray(p?.Endpoints) ? p.Endpoints : [];
      const cur = String(p?.CurAddr || "");
      const relay = typeof p?.Relay === "string" ? p.Relay : "";
      const curIsDerp = cur.toLowerCase().includes("derp");
      const directEndpoints = endpoints.filter((e: string) => !e.toLowerCase().includes("derp"));
      const privateEndpoints = directEndpoints.map(cleanAddr).filter((e: string) => PRIVATE.test(e));
      const pathDesc = relay
        ? `via DERP (${relay})`
        : curIsDerp
        ? `via DERP (${cur})`
        : privateEndpoints.length > 0
          ? `direct ${privateEndpoints[0]}`
          : directEndpoints.length > 0
            ? `direct ${directEndpoints[0]}`
            : cur
              ? `via ${cur}`
              : "unknown path";

      console.log("\n" + chalk.cyan.bold(name));
      const lines = [
        fmt("id:", p?.ID || p?.NodeID || p?.StableID),
        fmt("os:", p?.OS || p?.HostOS),
        fmt("user:", p?.User || p?.UserID),
        fmt("tags:", p?.Tags),
        fmt("online:", p?.Online ?? p?.Active),
        fmt("exit-node:", p?.ExitNode || p?.ExitNodeID || (p?.ExitNodeOption ? "yes" : undefined)),
        fmt("ts-ips:", tsIPs),
        fmt("lan-ips:", privateEndpoints),
        fmt("relay:", relay || undefined),
        fmt("cur-addr:", cur || undefined),
        fmt("endpoints:", endpoints),
        fmt("peer-api:", p?.PeerAPIURL),
        fmt("allowed-ips:", p?.AllowedIPs),
        fmt("path:", pathDesc),
        fmt("rx-bytes:", p?.RxBytes),
        fmt("tx-bytes:", p?.TxBytes),
        fmt("last-seen:", p?.LastSeen),
        fmt("key-expiry:", p?.KeyExpiry || p?.Expiry),
      ].filter(Boolean) as string[];
      for (const l of lines) console.log("  " + l);

      // Optional raw dump for full introspection
      if (RAW || lines.length < 3) {
        const minimal = {
          name,
          ...p,
        };
        console.log(chalk.gray("  raw:"));
        console.log(chalk.gray("  " + JSON.stringify(minimal, null, 2).split("\n").join("\n  ")));
      }
    }
  } catch (err) {
    // If status fails (not logged in, etc.), show version as a minimal test
    try {
      const { stdout } = await execFileP(tsPath, ["version"], { windowsHide: true });
      const line = String(stdout || "").trim().split(/\r?\n/)[0] || "(unknown version)";
      console.log(chalk.bold(`tailscale version: ${line}`));
    } catch (err2) {
      console.error(chalk.red("Failed to execute tailscale CLI."));
      if (err2 instanceof Error) console.error(chalk.dim(err2.message));
      process.exitCode = 1;
      return;
    }
  }
}

main().catch((e) => {
  console.error(chalk.red("Unexpected error:"));
  if (e instanceof Error) console.error(chalk.dim(e.stack || e.message));
  process.exitCode = 1;
});
