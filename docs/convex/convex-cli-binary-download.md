Below is exactly where the “download the Convex backend binary” logic lives today and what it does, step‑by‑step. This is the code path that runs when you do `npx convex dev` with a local backend (and more generally, whenever the CLI needs a local backend binary).

---

## Where the logic lives

> The Convex CLI is now in the **convex‑backend** monorepo, under `npm-packages/convex`. The download logic is in the “local deployment” helpers:

* **Binary download / selection logic**:
  `npm-packages/convex/src/cli/lib/localDeployment/download.ts` ([Glama – MCP Hosting Platform][1])
* **Cache and file layout (paths, executable name, etc.)**:
  `npm-packages/convex/src/cli/lib/localDeployment/filePaths.ts` ([Glama – MCP Hosting Platform][2])
* **Caller (where `npx convex dev` decides to fetch + run)**:
  `npm-packages/convex/src/cli/lib/localDeployment/localDeployment.ts` (calls `ensureBackendBinaryDownloaded`) and then spawns the binary via `run.ts`. ([Glama – MCP Hosting Platform][3])
* **Unit test showing selection of releases** (good reference for the algorithm/edge‑cases):
  `npm-packages/convex/src/cli/lib/localDeployment/run.test.ts` ([Glama – MCP Hosting Platform][4])

(Convex docs also confirm that the local backend runs as a subprocess of `npx convex dev` and keeps its state under `~/.convex`.) ([Convex Developer Hub][5])

---

## The exact algorithm the CLI uses

The function you care about is **`ensureBackendBinaryDownloaded`** plus its helper **`findLatestVersionWithBinary`** in `download.ts`. Here’s what they do:

1. **Choose the target artifact name for your platform** (`getDownloadPath()`):

   * macOS arm64 → `convex-local-backend-aarch64-apple-darwin.zip`
   * macOS x64 → `convex-local-backend-x86_64-apple-darwin.zip`
   * Linux arm64 → `convex-local-backend-aarch64-unknown-linux-gnu.zip`
   * Linux x64 → `convex-local-backend-x86_64-unknown-linux-gnu.zip`
   * Windows x64 → `convex-local-backend-x86_64-pc-windows-msvc.zip`
     If `process.platform`/`process.arch` don’t match those, the CLI errors out as “unsupported platform”. ([Glama – MCP Hosting Platform][1])

2. **Find the release tag to use** (`findLatestVersionWithBinary`):

   * Call **GitHub Releases API**:
     `GET https://api.github.com/repos/get-convex/convex-backend/releases?per_page=30`
     Follow pagination using the `Link` header (`rel="next"`) until done. ([Glama – MCP Hosting Platform][1])
   * Consider **only stable releases** (skip `draft` and `prerelease`). Track the “latest stable” tag encountered for messaging.
   * For each stable release page, check **whether any asset name exactly equals the platform artifact** from step 1.
   * Return the **first stable release tag** that contains the artifact.
   * If none of the stable releases contain it:

     * If there are no stable releases at all → crash with “Found no non‑draft, non‑prerelease…”.
     * Otherwise → crash with “Failed to find a convex backend release that contained <artifact>.”
   * A higher‑level caller may pass an “allowedVersion” (the one you already have on disk). If the GitHub query fails and an allowed version exists, the CLI **falls back** to that previously downloaded version instead of failing outright. ([Glama – MCP Hosting Platform][1])

3. **Check the cache** (`checkForExistingDownload`):

   * Look under **`~/.cache/convex/binaries/<version>/<exe>`** (see “Cache & layout” below).
   * If the version directory exists but **the executable file is missing**, the CLI **deletes the directory** to recover from partial/incomplete downloads.
   * On Unix (macOS/Linux), it runs `chmod +x` on the file to ensure it’s executable. ([Glama – MCP Hosting Platform][2])

4. **Download & unzip if needed** (`downloadBackendBinary` → `downloadZipFile`):

   * Construct URL:
     `https://github.com/get-convex/convex-backend/releases/download/<VERSION>/<FILENAME>`
     where `<VERSION>` is the tag from step 2 and `<FILENAME>` is from step 1. ([Glama – MCP Hosting Platform][1])
   * `fetch` the zip; if `Content-Length` is present and stdout is a TTY, show a **progress bar** using the `progress` package; otherwise print a “Downloading …” message.
   * Stream the body into a **temp file in a temp dir**, then unzip with `adm-zip` into another temp dir.
   * Move (`swapTmpFile`) the unzipped **`convex-local-backend[.exe]`** into the final versioned cache dir and **chmod +x** on Unix.
   * Return the final binary path. ([Glama – MCP Hosting Platform][1])

5. **Run & verify the binary** (`run.ts`):

   * The CLI does a **smoke test** by spawning the binary with `--help`.

     * If it exits with Windows error code **3221225781**, it prints a message about missing Visual C++ Redistributable.
     * Any non‑zero status is treated as failure with a detailed error message.
   * When starting for real, it spawns the local backend with flags like `--port`, `--site-proxy-port`, `--instance-name`, `--local-storage <dir>`, and the DB path `<.../convex_local_backend.sqlite3>`, then waits until `GET /instance_name` answers with the expected name. ([Glama – MCP Hosting Platform][6])

---

## Cache & file layout (what gets written where)

From `filePaths.ts` (and the top‑of‑file comment there):

* **Binary cache**: `~/.cache/convex/binaries/<version>/convex-local-backend[.exe]`
* **Dashboard cache** (the CLI can also download `dashboard.zip` similarly): `~/.cache/convex/dashboard/...`
* **Local state** for running the local backend: `~/.convex/convex-backend-state/<deployment>/...` (e.g. `convex_local_backend.sqlite3`, `convex_local_storage`).
* Executable name is platform‑specific: `convex-local-backend` on Unix, `convex-local-backend.exe` on Windows. ([Glama – MCP Hosting Platform][2])

> Note: the code uses a helper `cacheDir()` for the base cache path; on Unix that’s `~/.cache/convex`. (Windows/macOS are mapped appropriately by that helper.) The layout above reflects what the CLI expects. ([Glama – MCP Hosting Platform][2])

---

## Minimal, faithful re‑implementation (Node 18+)

Below is a compact drop‑in you can call from your own CLI. It mirrors the Convex CLI’s behavior: same platform mapping, GitHub Releases scanning, zip download, unzip, chmod, and versioned cache path.

> Requires: `npm i adm-zip` (uses Node’s built‑in `fetch` in Node 18+).

```ts
// convexBinary.ts
import fs from "fs";
import fsp from "fs/promises";
import os from "os";
import path from "path";
import AdmZip from "adm-zip";
import { Readable } from "stream";

const OWNER = "get-convex";
const REPO = "convex-backend";
const LIST_URL = `https://api.github.com/repos/${OWNER}/${REPO}/releases?per_page=30`;

export async function ensureConvexBinary(opts?: {
  version?: string;              // if omitted, find latest stable with asset
  fallbackVersionIfOffline?: string; // optional "allowedVersion" fallback
  userAgent?: string;            // optional UA for GitHub API
}): Promise<{ version: string; binaryPath: string }> {
  const artifact = getArtifactNameForThisPlatform();
  if (!artifact) {
    throw new Error(`Unsupported platform ${process.platform} ${process.arch}`);
  }

  const version = opts?.version ?? await findLatestVersionWithBinary(artifact, {
    userAgent: opts?.userAgent,
    fallbackVersion: opts?.fallbackVersionIfOffline,
  });

  const dest = getVersionedBinaryPath(version);
  if (await exists(dest)) {
    await makeExecutable(dest);
    return { version, binaryPath: dest };
  }

  await downloadAndInstall(version, artifact);
  await makeExecutable(dest);
  return { version, binaryPath: dest };
}

function getArtifactNameForThisPlatform(): string | null {
  switch (process.platform) {
    case "darwin":
      return process.arch === "arm64"
        ? "convex-local-backend-aarch64-apple-darwin.zip"
        : process.arch === "x64"
        ? "convex-local-backend-x86_64-apple-darwin.zip"
        : null;
    case "linux":
      return process.arch === "arm64"
        ? "convex-local-backend-aarch64-unknown-linux-gnu.zip"
        : process.arch === "x64"
        ? "convex-local-backend-x86_64-unknown-linux-gnu.zip"
        : null;
    case "win32":
      return "convex-local-backend-x86_64-pc-windows-msvc.zip";
    default:
      return null;
  }
}

async function findLatestVersionWithBinary(
  artifact: string,
  opts: { userAgent?: string; fallbackVersion?: string }
): Promise<string> {
  let url: string | null = LIST_URL;
  let latestStable: string | null = null;

  try {
    while (url) {
      const res = await fetch(url, {
        headers: { "User-Agent": opts.userAgent ?? "convex-binary-fetcher" },
      });
      if (!res.ok) {
        throw new Error(`GitHub API ${res.status}: ${await res.text()}`);
      }
      const releases: any[] = await res.json();

      for (const r of releases) {
        if (!latestStable && !r.draft && !r.prerelease) {
          latestStable = r.tag_name;
        }
        if (!r.draft && !r.prerelease) {
          if (Array.isArray(r.assets) && r.assets.some((a: any) => a?.name === artifact)) {
            return r.tag_name;
          }
        }
      }

      url = parseLinkHeader(res.headers.get("Link"))?.next ?? null;
    }

    if (!latestStable) {
      throw new Error("Found no non-draft, non-prerelease convex backend releases.");
    }
    throw new Error(`Failed to find a convex backend release that contained ${artifact}.`);
  } catch (err) {
    if (opts.fallbackVersion) {
      // Mirror Convex CLI behavior: log & fall back to a previously-downloaded version if given
      console.warn(
        `Failed to get latest release info (${(err as Error).message}); using downloaded version ${opts.fallbackVersion}`
      );
      return opts.fallbackVersion;
    }
    throw err;
  }
}

function parseLinkHeader(h: string | null): { next?: string } | null {
  if (!h) return null;
  const out: Record<string, string> = {};
  for (const part of h.split(",")) {
    const [urlPart, relPart] = part.split(";");
    if (!urlPart || !relPart) continue;
    const url = urlPart.trim().replace(/^<|>$/g, "");
    const rel = relPart.trim().replace(/^rel="|"$|rel=/g, "").replace(/"/g, "");
    out[rel] = url;
  }
  return out;
}

async function downloadAndInstall(version: string, artifact: string) {
  const url = `https://github.com/${OWNER}/${REPO}/releases/download/${version}/${artifact}`;
  const res = await fetch(url);
  if (res.status !== 200) {
    throw new Error(`File not found at ${url}`);
  }

  const tmpRoot = await fsp.mkdtemp(path.join(os.tmpdir(), "convex-"));
  const zipPath = path.join(tmpRoot, "backend.zip");

  // stream body to disk
  const body = res.body as unknown as Readable;
  await new Promise<void>((resolve, reject) => {
    const ws = fs.createWriteStream(zipPath);
    body.pipe(ws);
    body.on("error", reject);
    ws.on("finish", () => resolve());
    ws.on("error", reject);
  });

  // unzip
  const zip = new AdmZip(zipPath);
  const unzipDir = path.join(tmpRoot, "unzipped");
  await fsp.mkdir(unzipDir, { recursive: true });
  zip.extractAllTo(unzipDir, true);

  const exeName = process.platform === "win32" ? "convex-local-backend.exe" : "convex-local-backend";
  const exeSrc = path.join(unzipDir, exeName);
  const dest = getVersionedBinaryPath(version);

  await fsp.mkdir(path.dirname(dest), { recursive: true });
  await fsp.copyFile(exeSrc, dest);
}

function getVersionedBinaryPath(version: string): string {
  const base = getConvexCacheDir(); // mirrors CLI's ~/.cache/convex
  return path.join(base, "binaries", version, process.platform === "win32" ? "convex-local-backend.exe" : "convex-local-backend");
}

function getConvexCacheDir(): string {
  // Mirrors CLI layout: ~/.cache/convex on Unix; platform-appropriate elsewhere.
  if (process.platform === "win32") {
    const localAppData = process.env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local");
    return path.join(localAppData, "convex");
  }
  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Caches", "convex");
  }
  const xdg = process.env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache");
  return path.join(xdg, "convex");
}

async function exists(p: string): Promise<boolean> {
  try {
    await fsp.access(p, fs.constants.F_OK);
    return true;
  } catch {
    return false;
  }
}

async function makeExecutable(p: string) {
  if (process.platform !== "win32") {
    await fsp.chmod(p, 0o755);
  }
}
```

**Usage from your CLI:**

```ts
import { ensureConvexBinary } from "./convexBinary";

(async () => {
  const { version, binaryPath } = await ensureConvexBinary({
    // version: "precompiled-2025-01-31-e52353b", // optional: pin a tag
    // fallbackVersionIfOffline: "precompiled-2025-01-31-e52353b",
    userAgent: "my-cli (me@example.com)",
  });
  console.log("Using Convex backend", version, "at", binaryPath);
})();
```

This reproduces the behavior in `download.ts`: identical artifact name mapping; scans GitHub Releases with pagination; downloads the zipped asset; extracts the `convex-local-backend[.exe]`; stores it under a versioned cache directory; marks it executable on Unix; and returns the path. ([Glama – MCP Hosting Platform][1])

---

## Practical notes and edge cases

* **Release tags & assets**: The release tags look like `precompiled-YYYY-MM-DD-<shortrev>`, and each stable release exposes the platform‑specific `convex-local-backend-<target>.zip` assets. The test fixture shows exactly how tags are compared and assets are matched. ([Glama – MCP Hosting Platform][4])
* **Progress bar**: The official CLI shows a progress bar when `Content-Length` is present and the terminal is a TTY; otherwise it prints a plain “Downloading …” message. (Above, we keep it simple, but you can add `progress` if you want parity.) ([Glama – MCP Hosting Platform][1])
* **Validation before running**: The CLI runs the binary with `--help` to catch missing DLLs (Windows code `3221225781`) and other problems before trying to start the server. If you also plan to spawn it, consider the same pre‑check. ([Glama – MCP Hosting Platform][6])
* **Where state goes**: The local backend’s runtime state lives under `~/.convex/convex-backend-state/<deployment>/…` and the CLI waits for `GET /instance_name` to match before proceeding. ([Convex Developer Hub][5])

---

## TL;DR algorithm (copyable checklist)

1. Map `(process.platform, process.arch)` → the asset filename. ([Glama – MCP Hosting Platform][1])
2. If `version` not provided, call GitHub Releases API, paginate via `Link`, pick the **first stable release** that contains that asset. ([Glama – MCP Hosting Platform][1])
3. Look for `~/.cache/convex/binaries/<version>/convex-local-backend[.exe]`. If present, `chmod +x` (Unix) and use it; else continue. ([Glama – MCP Hosting Platform][2])
4. Download `https://github.com/get-convex/convex-backend/releases/download/<version>/<asset>.zip`, stream to a temp file, unzip, move the contained executable into that versioned cache dir, `chmod +x` (Unix). ([Glama – MCP Hosting Platform][1])
5. (Optional) Smoke‑test the binary (`--help`) and handle Windows redistributable error if needed. ([Glama – MCP Hosting Platform][6])

If you follow the code above, your CLI will behave the same way `npx convex dev` does when it needs to fetch and run the Convex local backend.

[1]: https://glama.ai/mcp/servers/%40get-convex/convex-backend/blob/e41b74fbc3d03bc14f9bd5eb1256132c1a48b05a/npm-packages/convex/src/cli/lib/localDeployment/download.ts "Convex MCP server | Glama"
[2]: https://glama.ai/mcp/servers/%40get-convex/convex-backend/blob/e41b74fbc3d03bc14f9bd5eb1256132c1a48b05a/npm-packages/convex/src/cli/lib/localDeployment/filePaths.ts "Convex MCP server | Glama"
[3]: https://glama.ai/mcp/servers/%40get-convex/convex-backend/blob/e41b74fbc3d03bc14f9bd5eb1256132c1a48b05a/npm-packages/convex/src/cli/lib/localDeployment/localDeployment.ts "Convex MCP server | Glama"
[4]: https://glama.ai/mcp/servers/%40get-convex/convex-backend/blob/e41b74fbc3d03bc14f9bd5eb1256132c1a48b05a/npm-packages/convex/src/cli/lib/localDeployment/run.test.ts "Convex MCP server | Glama"
[5]: https://docs.convex.dev/cli/local-deployments "Local Deployments for Development | Convex Developer Hub"
[6]: https://glama.ai/mcp/servers/%40get-convex/convex-backend/blob/e41b74fbc3d03bc14f9bd5eb1256132c1a48b05a/npm-packages/convex/src/cli/lib/localDeployment/run.ts "Convex MCP server | Glama"
