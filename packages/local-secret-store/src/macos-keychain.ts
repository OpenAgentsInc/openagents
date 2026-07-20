/**
 * IDR-05 macOS Keychain secret-store adapter — the REAL, OWNER-ATTENDED path.
 *
 * This is the version-one adapter that is genuinely built and run on this
 * machine. It drives the macOS `security` generic-password commands. Two rules
 * make it safe for an autonomous workspace:
 *
 * 1. The secret NEVER rides in `argv`. `security add-generic-password` receives
 *    the payload (base64) on stdin, so the phrase can never appear in `ps`, a
 *    shell history, or a log. A pure test asserts the built argv contains no
 *    payload bytes.
 * 2. The real `security` tool runs ONLY behind an explicit owner-attended gate.
 *    `macosKeychainOwnerAttendedLayer` wires the real spawning runner ONLY when
 *    the caller acknowledges the owner-attended intent in code AND the process is
 *    macOS AND the owner-attended environment flag is set. In every other case —
 *    above all the automated test suite, which sets none of these — it composes
 *    the FAIL-CLOSED runner, so no `security` command, and therefore no Keychain
 *    password dialog, can ever run unattended.
 *
 * The automated suite proves the adapter's orchestration by injecting a
 * deterministic fake runner into `commandBackedLocalSecretStore`; it never calls
 * `macosKeychainOwnerAttendedLayer` with the gate open.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Buffer } from "node:buffer";
import process from "node:process";
import { Layer } from "effect";
import {
  commandBackedLocalSecretStore,
  commandBackedLocalSecretStoreLayer,
  type PlatformCommandSpec,
  type ReadInterpretation,
} from "./command-backed-store.ts";
import {
  failClosedSecretStoreCommandRunner,
  nodeSpawnSecretStoreCommandRunner,
  type SecretStoreCommandRunnerInterface,
} from "./command-runner.ts";
import { LocalSecretStore } from "./secret-store.ts";

/** The macOS `security` tool path. */
export const MACOS_SECURITY_EXECUTABLE = "/usr/bin/security";

/** The `security` exit code for `errSecItemNotFound`. */
export const MACOS_ITEM_NOT_FOUND_CODE = 44;

const toBase64Stdin = (payload: Uint8Array): Uint8Array =>
  new Uint8Array(Buffer.from(Buffer.from(payload).toString("base64"), "utf8"));

const fromBase64Stdout = (stdout: Uint8Array): Uint8Array =>
  new Uint8Array(Buffer.from(Buffer.from(stdout).toString("utf8").trim(), "base64"));

/**
 * The macOS Keychain command spec. Every builder is pure. The write command
 * carries the payload on stdin (base64), never in argv.
 */
export const macosKeychainCommandSpec: PlatformCommandSpec = {
  platformKind: "macos_keychain",
  buildSet: (locator, payload) => ({
    executable: MACOS_SECURITY_EXECUTABLE,
    // `-U` updates an existing item instead of failing. `-w` with no inline value
    // makes `security` take the password from stdin, so it stays off argv.
    argv: ["add-generic-password", "-U", "-s", locator.service, "-a", locator.account, "-w"],
    stdin: toBase64Stdin(payload),
  }),
  buildGet: (locator) => ({
    executable: MACOS_SECURITY_EXECUTABLE,
    argv: ["find-generic-password", "-s", locator.service, "-a", locator.account, "-w"],
  }),
  buildDelete: (locator) => ({
    executable: MACOS_SECURITY_EXECUTABLE,
    argv: ["delete-generic-password", "-s", locator.service, "-a", locator.account],
  }),
  buildPresence: (locator) => ({
    executable: MACOS_SECURITY_EXECUTABLE,
    argv: ["find-generic-password", "-s", locator.service, "-a", locator.account],
  }),
  interpretSet: (result) => result.code === 0,
  interpretGet: (result): ReadInterpretation | "error" => {
    if (result.code === MACOS_ITEM_NOT_FOUND_CODE) return { _tag: "not_found" };
    if (result.code !== 0) return "error";
    return { _tag: "found", payload: fromBase64Stdout(result.stdout) };
  },
  // A missing item on delete is a success: delete is idempotent.
  interpretDelete: (result) => result.code === 0 || result.code === MACOS_ITEM_NOT_FOUND_CODE,
  interpretPresence: (result) => result.code === 0,
};

/**
 * Build a macOS Keychain `LocalSecretStore` over an injected runner. The
 * automated suite injects a fake runner; an owner-attended run injects the real
 * spawning runner through the gated layer below. This function itself never
 * spawns anything.
 */
export const makeMacosKeychainLocalSecretStore = (runner: SecretStoreCommandRunnerInterface) =>
  commandBackedLocalSecretStore(macosKeychainCommandSpec, runner);

/** The explicit owner-attended intent for the real macOS Keychain path. */
export interface MacosKeychainOwnerAttendedOptions {
  /** The caller MUST set this to `true` on purpose to open the real path. */
  readonly acknowledgeOwnerAttended: true;
}

/**
 * The environment flag that must be `"1"` for the real macOS Keychain path to
 * open. An owner sets it during an attended run. The automated suite never sets
 * it, so the real `security` tool can never run unattended even if this factory
 * were called by accident.
 */
export const MACOS_KEYCHAIN_ATTENDED_ENV = "OPENAGENTS_IDENTITY_KEYCHAIN_ATTENDED";

/** Whether the owner-attended gate for the real macOS Keychain path is open. */
export const macosKeychainAttendedGateOpen = (
  options: MacosKeychainOwnerAttendedOptions,
): boolean =>
  options.acknowledgeOwnerAttended === true &&
  process.platform === "darwin" &&
  process.env[MACOS_KEYCHAIN_ATTENDED_ENV] === "1";

/**
 * The macOS Keychain layer. It wires the REAL spawning runner ONLY when the
 * owner-attended gate is open (code acknowledgement + macOS + environment flag).
 * In every other case it composes the FAIL-CLOSED runner, so no `security`
 * command can run. The automated suite therefore never opens a Keychain dialog.
 */
export const macosKeychainOwnerAttendedLayer = (
  options: MacosKeychainOwnerAttendedOptions,
): Layer.Layer<LocalSecretStore> =>
  macosKeychainAttendedGateOpen(options)
    ? commandBackedLocalSecretStoreLayer(
        macosKeychainCommandSpec,
        nodeSpawnSecretStoreCommandRunner,
      )
    : Layer.succeed(
        LocalSecretStore,
        LocalSecretStore.of(
          commandBackedLocalSecretStore(
            macosKeychainCommandSpec,
            failClosedSecretStoreCommandRunner,
          ),
        ),
      );
