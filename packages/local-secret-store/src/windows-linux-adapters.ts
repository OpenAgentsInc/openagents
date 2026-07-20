/**
 * IDR-05 Windows Credential Manager and Linux Secret Service — TYPED adapters.
 *
 * These two desktop platform stores share the macOS command-backed pattern, but
 * they are TYPED adapters here: their command specs and gated owner-attended
 * layers are defined and unit-tested (builder shape, no-secret-in-argv, exit-code
 * interpretation) through the injected-runner core, and their REAL OS tool runs
 * only in an owner-attended run on that platform. This workspace is macOS, so the
 * real Windows/Linux paths are proven on those hosts, not in this suite.
 *
 * Both keep the secret OFF `argv`: the Linux `secret-tool store` reads the
 * payload from stdin, and the Windows PowerShell `Set-Secret` script reads the
 * payload from `[Console]::In`. The automated suite never opens either gate.
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
} from "./command-runner.ts";
import { MACOS_KEYCHAIN_ATTENDED_ENV } from "./macos-keychain.ts";
import { LocalSecretStore } from "./secret-store.ts";

const encodeBase64Stdin = (payload: Uint8Array): Uint8Array =>
  new Uint8Array(Buffer.from(Buffer.from(payload).toString("base64"), "utf8"));

const decodeBase64Stdout = (stdout: Uint8Array): Uint8Array =>
  new Uint8Array(Buffer.from(Buffer.from(stdout).toString("utf8").trim(), "base64"));

// ---------------------------------------------------------------------------
// Linux Secret Service (libsecret `secret-tool`)
// ---------------------------------------------------------------------------

/** The Linux `secret-tool` executable. */
export const LINUX_SECRET_TOOL_EXECUTABLE = "secret-tool";

/** The public label a stored Linux Secret Service entry carries. */
export const LINUX_SECRET_LABEL = "OpenAgents local identity root";

/** `secret-tool` exits non-zero when a lookup finds no entry. */
export const LINUX_NOT_FOUND_CODE = 1;

/**
 * The Linux Secret Service command spec. `secret-tool store` reads the payload
 * (base64) from stdin, so the secret is never on argv. The service and account
 * are stored as libsecret attributes.
 */
export const linuxSecretServiceCommandSpec: PlatformCommandSpec = {
  platformKind: "linux_secret_service",
  buildSet: (locator, payload) => ({
    executable: LINUX_SECRET_TOOL_EXECUTABLE,
    argv: [
      "store",
      `--label=${LINUX_SECRET_LABEL}`,
      "service",
      locator.service,
      "account",
      locator.account,
    ],
    stdin: encodeBase64Stdin(payload),
  }),
  buildGet: (locator) => ({
    executable: LINUX_SECRET_TOOL_EXECUTABLE,
    argv: ["lookup", "service", locator.service, "account", locator.account],
  }),
  buildDelete: (locator) => ({
    executable: LINUX_SECRET_TOOL_EXECUTABLE,
    argv: ["clear", "service", locator.service, "account", locator.account],
  }),
  buildPresence: (locator) => ({
    executable: LINUX_SECRET_TOOL_EXECUTABLE,
    argv: ["lookup", "service", locator.service, "account", locator.account],
  }),
  interpretSet: (result) => result.code === 0,
  interpretGet: (result): ReadInterpretation | "error" => {
    if (result.code === 0) return { _tag: "found", payload: decodeBase64Stdout(result.stdout) };
    return { _tag: "not_found" };
  },
  // `secret-tool clear` succeeds whether or not a matching entry existed.
  interpretDelete: (result) => result.code === 0 || result.code === LINUX_NOT_FOUND_CODE,
  interpretPresence: (result) => result.code === 0,
};

// ---------------------------------------------------------------------------
// Windows Credential Manager (PowerShell SecretManagement)
// ---------------------------------------------------------------------------

/** The Windows PowerShell executable the adapter drives. */
export const WINDOWS_POWERSHELL_EXECUTABLE = "powershell";

/** A Windows cmdlet error (for example a missing secret) exits non-zero. */
export const WINDOWS_NOT_FOUND_CODE = 1;

/** Build the single-string SecretManagement name for one locator. */
export const windowsSecretName = (service: string, account: string): string =>
  `${service}::${account}`;

const powershellCommand = (script: string): ReadonlyArray<string> => [
  "-NoProfile",
  "-NonInteractive",
  "-Command",
  script,
];

/**
 * The Windows Credential Manager command spec. The write script reads the
 * payload (base64) from `[Console]::In`, so the secret is never on argv; the
 * script text on argv names only the SecretManagement operation and the locator.
 */
export const windowsCredentialManagerCommandSpec: PlatformCommandSpec = {
  platformKind: "windows_credential_manager",
  buildSet: (locator, payload) => ({
    executable: WINDOWS_POWERSHELL_EXECUTABLE,
    argv: powershellCommand(
      `$payload = [Console]::In.ReadToEnd(); Set-Secret -Name '${windowsSecretName(
        locator.service,
        locator.account,
      )}' -Secret $payload`,
    ),
    stdin: encodeBase64Stdin(payload),
  }),
  buildGet: (locator) => ({
    executable: WINDOWS_POWERSHELL_EXECUTABLE,
    argv: powershellCommand(
      `Get-Secret -Name '${windowsSecretName(locator.service, locator.account)}' -AsPlainText`,
    ),
  }),
  buildDelete: (locator) => ({
    executable: WINDOWS_POWERSHELL_EXECUTABLE,
    argv: powershellCommand(
      `Remove-Secret -Name '${windowsSecretName(locator.service, locator.account)}'`,
    ),
  }),
  buildPresence: (locator) => ({
    executable: WINDOWS_POWERSHELL_EXECUTABLE,
    argv: powershellCommand(
      `if (Get-SecretInfo -Name '${windowsSecretName(
        locator.service,
        locator.account,
      )}') { exit 0 } else { exit 1 }`,
    ),
  }),
  interpretSet: (result) => result.code === 0,
  interpretGet: (result): ReadInterpretation | "error" => {
    if (result.code === 0) return { _tag: "found", payload: decodeBase64Stdout(result.stdout) };
    return { _tag: "not_found" };
  },
  interpretDelete: (result) => result.code === 0 || result.code === WINDOWS_NOT_FOUND_CODE,
  interpretPresence: (result) => result.code === 0,
};

// ---------------------------------------------------------------------------
// Owner-attended gated layers
// ---------------------------------------------------------------------------

const gatedDesktopLayer = (
  spec: PlatformCommandSpec,
  platform: NodeJS.Platform,
  acknowledgeOwnerAttended: boolean,
): Layer.Layer<LocalSecretStore> => {
  const gateOpen =
    acknowledgeOwnerAttended === true &&
    process.platform === platform &&
    process.env[MACOS_KEYCHAIN_ATTENDED_ENV] === "1";
  return gateOpen
    ? commandBackedLocalSecretStoreLayer(spec, nodeSpawnSecretStoreCommandRunner)
    : Layer.succeed(
        LocalSecretStore,
        LocalSecretStore.of(
          commandBackedLocalSecretStore(spec, failClosedSecretStoreCommandRunner),
        ),
      );
};

/** The explicit owner-attended intent for a real desktop secret-store path. */
export interface DesktopSecretStoreOwnerAttendedOptions {
  /** The caller MUST set this to `true` on purpose to open the real path. */
  readonly acknowledgeOwnerAttended: true;
}

/**
 * The Linux Secret Service layer. It wires the real `secret-tool` runner ONLY on
 * Linux with the owner-attended acknowledgement and environment flag; otherwise
 * it is fail-closed.
 */
export const linuxSecretServiceOwnerAttendedLayer = (
  options: DesktopSecretStoreOwnerAttendedOptions,
): Layer.Layer<LocalSecretStore> =>
  gatedDesktopLayer(linuxSecretServiceCommandSpec, "linux", options.acknowledgeOwnerAttended);

/**
 * The Windows Credential Manager layer. It wires the real PowerShell runner ONLY
 * on Windows with the owner-attended acknowledgement and environment flag;
 * otherwise it is fail-closed.
 */
export const windowsCredentialManagerOwnerAttendedLayer = (
  options: DesktopSecretStoreOwnerAttendedOptions,
): Layer.Layer<LocalSecretStore> =>
  gatedDesktopLayer(windowsCredentialManagerCommandSpec, "win32", options.acknowledgeOwnerAttended);
