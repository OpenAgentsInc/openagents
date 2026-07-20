/**
 * IDR-05 OS-command runner port for the desktop platform secret stores.
 *
 * A desktop platform secret store (macOS Keychain, Windows Credential Manager,
 * Linux Secret Service) is reached through its OS command-line tool. This module
 * defines the ONE thing those adapters run: a `SecretStoreCommand`. The secret
 * bytes, when present, travel on `stdin` and NEVER in `argv`, so the payload can
 * never appear in the process argument list, a shell history, `ps` output, or a
 * log line.
 *
 * The module ships two runners:
 *
 * - `failClosedSecretStoreCommandRunner`: every command fails
 *   `adapter_unavailable` and touches no process. The automated test suite and
 *   any non-owner-attended host compose this, so no OS keychain tool ever runs.
 * - `nodeSpawnSecretStoreCommandRunner`: spawns the real OS tool through
 *   `node:child_process`. It is the ONLY runner that touches a real platform
 *   store, so a platform adapter wires it ONLY behind an explicit
 *   owner-attended gate.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Effect } from "effect";
import { SecretStoreError } from "./secret-store.ts";

/**
 * One OS command a desktop secret-store adapter runs. Any secret bytes go on
 * `stdin`; `argv` carries only the service and account locator fields and the
 * tool flags, never the secret.
 */
export interface SecretStoreCommand {
  /** The OS tool to run, for example `/usr/bin/security` or `secret-tool`. */
  readonly executable: string;
  /** The tool arguments. It never contains the secret payload. */
  readonly argv: ReadonlyArray<string>;
  /** The secret bytes, fed on stdin. It is `undefined` for a read or delete. */
  readonly stdin?: Uint8Array;
}

/** The result of one OS command. The stdout carries a read payload, base64 or raw. */
export interface SecretStoreCommandResult {
  /** The process exit code. */
  readonly code: number;
  /** The captured stdout bytes. */
  readonly stdout: Uint8Array;
}

/**
 * The OS-command runner port. A platform adapter injects one. A real runner
 * spawns the OS tool; a test injects a deterministic fake so no OS tool runs.
 */
export interface SecretStoreCommandRunnerInterface {
  readonly run: (
    command: SecretStoreCommand,
  ) => Effect.Effect<SecretStoreCommandResult, SecretStoreError>;
}

/**
 * The fail-closed runner. Every command fails `adapter_unavailable` and touches
 * no process. A host composes it whenever the owner-attended gate is not open,
 * so no OS keychain tool can run in an autonomous context.
 */
export const failClosedSecretStoreCommandRunner: SecretStoreCommandRunnerInterface = {
  run: () => Effect.fail(new SecretStoreError({ reason: "adapter_unavailable" })),
};

/**
 * The real OS-command runner. It spawns the tool through `node:child_process`,
 * writes any `stdin` bytes to the child, and collects stdout. It never writes
 * the secret to a log. This is the ONLY runner that touches a real platform
 * store, so an adapter wires it ONLY behind an explicit owner-attended gate.
 */
export const nodeSpawnSecretStoreCommandRunner: SecretStoreCommandRunnerInterface = {
  run: (command) =>
    Effect.tryPromise({
      try: async () => {
        const { spawn } = await import("node:child_process");
        return await new Promise<SecretStoreCommandResult>((resolve, reject) => {
          const child = spawn(command.executable, [...command.argv], {
            stdio: ["pipe", "pipe", "ignore"],
          });
          const chunks: Array<Uint8Array> = [];
          child.stdout.on("data", (chunk: Uint8Array) => chunks.push(chunk));
          child.on("error", (error) => reject(error));
          child.on("close", (code) => {
            const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
            const stdout = new Uint8Array(total);
            let offset = 0;
            for (const chunk of chunks) {
              stdout.set(chunk, offset);
              offset += chunk.length;
            }
            resolve({ code: code ?? -1, stdout });
          });
          if (command.stdin !== undefined) {
            child.stdin.write(command.stdin);
          }
          child.stdin.end();
        });
      },
      catch: () => new SecretStoreError({ reason: "storage_unavailable" }),
    }),
};
