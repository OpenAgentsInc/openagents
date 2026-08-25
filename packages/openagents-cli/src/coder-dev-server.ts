import { execFileSync, spawn } from "node:child_process";
import { existsSync, openSync, readFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

/**
 * Starting and waiting on the `openagents.com` development server for `--dev`.
 *
 * `--dev` exists because a deploy can take half an hour. Telling the reader to
 * go and start a server themselves puts a second wait in front of the first
 * one, so this starts it: a session that asks for the dev lane gets the dev
 * lane, and the only thing it has to be told is which log to read if the server
 * does not come up.
 *
 * The server is left running when the session ends. It compiles on boot and
 * that cost is worth paying once rather than once per session, and a reader who
 * wants it gone knows where it is.
 */

/** Where the server's output goes, so a failed boot can be read rather than guessed at. */
export const devServerLog = (): string => join(tmpdir(), "openagents-dev-server.log");

/**
 * The `openagents.com` checkout to start a server from.
 *
 * Walks up from the working directory first, so a session already inside the
 * repository starts that copy rather than one somewhere else on the machine.
 * `OPENAGENTS_COM_PATH` overrides for a checkout in neither place.
 */
export const findSiteCheckout = (
  from: string = process.cwd(),
  env: NodeJS.ProcessEnv = process.env,
): string | undefined => {
  const named = env.OPENAGENTS_COM_PATH;
  if (named !== undefined && siteAppRoot(named)) return named;

  let at = resolve(from);
  for (;;) {
    if (siteAppRoot(at)) return at;
    const up = dirname(at);
    if (up === at) break;
    at = up;
  }

  // A conventional checkout in the home workspace, when the session was
  // started outside both repositories.
  const common = join(homedir(), "work", "openagents.com");
  return siteAppRoot(common) ? common : undefined;
};

/** True when a directory is the root of the `openagents.com` Phoenix application. */
const siteAppRoot = (path: string): boolean => {
  const mix = join(path, "mix.exs");
  if (!existsSync(mix)) return false;
  try {
    const contents = readFileSync(mix, "utf8");
    return contents.includes("app: :openagents") && existsSync(join(path, "lib", "openagents_web"));
  } catch {
    return false;
  }
};

/** What `/healthz` says: serving, needs migrations, or not answering. */
type Health = "ok" | "pending_migrations" | "down";

const health = async (origin: string, timeoutMs = 1_500): Promise<Health> => {
  try {
    const answer = await fetch(new URL("/healthz", origin), {
      signal: AbortSignal.timeout(timeoutMs),
    });
    const body = await answer.text();
    if (answer.ok && body.includes(`"status"`)) return "ok";
    // Phoenix serves the pending-migration error as its own debug page, which
    // is a live server that cannot answer yet rather than a dead one.
    return body.includes("PendingMigrationError") ? "pending_migrations" : "down";
  } catch {
    return "down";
  }
};

/** Whether a server is already serving at this origin. */
export const devServerReady = async (origin: string): Promise<boolean> =>
  (await health(origin)) === "ok";

/** Extract the port number from a target origin URL, default 4000 for standard local dev. */
export const originPort = (origin: string): number | undefined => {
  try {
    const url = new URL(origin);
    if (url.port) return Number.parseInt(url.port, 10);
    if (url.protocol === "http:") return 80;
    if (url.protocol === "https:") return 443;
    return undefined;
  } catch {
    return undefined;
  }
};

/** Find all PIDs listening on a given TCP port. */
export const listeningPids = (port: number): number[] => {
  try {
    const stdout = execFileSync("lsof", ["-ti", `tcp:${port}`], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return stdout
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0)
      .map((pid) => Number.parseInt(pid, 10))
      .filter((pid) => !Number.isNaN(pid) && pid !== process.pid);
  } catch {
    return [];
  }
};

/** Kill processes that occupy a port when they are unresponsive to health checks. */
export const killPortOccupants = (port: number): number[] => {
  const pids = listeningPids(port);
  for (const pid of pids) {
    try {
      process.kill(pid, "SIGTERM");
    } catch {
      // Process may already be dead or owned by another user.
    }
  }
  // Brief pause before SIGKILL for stubborn hung processes
  if (pids.length > 0) {
    for (const pid of pids) {
      try {
        process.kill(pid, "SIGKILL");
      } catch {
        // Ignored
      }
    }
  }
  return pids;
};

const run = (command: string, args: readonly string[], cwd: string, log: number) =>
  new Promise<number>((settle) => {
    const child = spawn(command, args, {
      cwd,
      stdio: ["ignore", log, log],
      env: { ...process.env, MIX_ENV: "dev" },
    });
    child.on("close", (code) => settle(code ?? 1));
    child.on("error", () => settle(1));
  });

export interface DevServerStart {
  readonly started: boolean;
  /** Why it could not be started, for a reader who has to act on it. */
  readonly refusal?: string;
}

/**
 * Bring a development server up at this origin, or report why not.
 *
 * Already serving is success and starts nothing. Otherwise it starts one in the
 * checkout it finds, cleans up any unresponsive process occupying the port,
 * migrates when the server says its database is behind, and waits for `/healthz`
 * to answer — a first boot compiles, so the wait is long and says so rather
 * than looking like a hang.
 */
export const startDevServer = async (
  origin: string,
  options: {
    readonly notice?: (message: string) => void;
    readonly onProgress?: (elapsedMs: number) => void;
    readonly timeoutMs?: number;
    readonly cwd?: string;
  } = {},
): Promise<DevServerStart> => {
  const notice = options.notice ?? (() => undefined);
  const onProgress = options.onProgress ?? (() => undefined);
  const start = Date.now();
  const deadline = start + (options.timeoutMs ?? 240_000);

  const current = await health(origin);
  if (current === "ok") return { started: false };

  const checkout = findSiteCheckout(options.cwd);
  if (checkout === undefined) {
    return {
      started: false,
      refusal:
        `No development server is answering at ${origin}, and no openagents.com ` +
        "checkout was found to start one from. Set OPENAGENTS_COM_PATH to it, or drop --dev.",
    };
  }

  const logPath = devServerLog();
  const log = openSync(logPath, "a");

  // If health check failed ("down"), check if an unresponsive process is already
  // squatting on the target port and holding locks or preventing binding.
  const port = originPort(origin);
  if (port !== undefined) {
    const killed = killPortOccupants(port);
    if (killed.length > 0) {
      notice(`Terminated unresponsive process (${killed.join(", ")}) on port ${port}.`);
    }
  }

  // A server that is up but refusing every request because its database is
  // behind is one migration away from working, and running it is what a person
  // would do next anyway.
  if (current === "pending_migrations") {
    notice("The dev server's database is behind. Migrating.");
    await run("mix", ["ecto.migrate"], checkout, log);
    if (await devServerReady(origin)) return { started: true };
  }

  notice(`Starting a dev server in ${checkout}. First boot compiles, so this can take a minute.`);

  // Detached, so the server outlives this session: the next `--dev` finds it
  // already up and pays no boot cost at all.
  const server = spawn("mix", ["phx.server"], {
    cwd: checkout,
    stdio: ["ignore", log, log],
    detached: true,
    env: { ...process.env, MIX_ENV: "dev" },
  });
  server.unref();

  let migrated = false;
  for (;;) {
    onProgress(Date.now() - start);
    if (Date.now() > deadline) {
      return {
        started: false,
        refusal:
          `A dev server was started in ${checkout} but did not answer at ${origin} in time. ` +
          `Its output is in ${logPath}.`,
      };
    }

    await new Promise((wake) => setTimeout(wake, 1_000));
    const state = await health(origin);
    if (state === "ok") return { started: true };

    if (state === "pending_migrations" && !migrated) {
      migrated = true;
      notice("The dev server's database is behind. Migrating.");
      await run("mix", ["ecto.migrate"], checkout, log);
    }
  }
};
