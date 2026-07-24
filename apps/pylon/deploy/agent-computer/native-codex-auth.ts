#!/usr/bin/env node

import {
  chmodSync,
  closeSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";

import { Schema } from "effect";

import { openCodexAppServerClient } from "../../../openagents-desktop/src/codex-app-server-client.ts";

export const NATIVE_CODEX_AUTH_MAX_BYTES = 256 * 1024;
export const NATIVE_CODEX_AUTH_DEFAULT_TIMEOUT_MS = 20_000;
export const NATIVE_CODEX_AUTH_MARKER = ".openagents-native-codex-scratch.json";
export const CODEX_NATIVE_AUTH_IMPORT_PATH =
  "/api/operator/provider-accounts/chatgpt-codex/native-auth/import";
export const CODEX_LEASE_ACQUIRE_PATH = "/api/operator/provider-accounts/chatgpt-codex/leases";
export const CODEX_LEASE_GRANT_PATH = "/api/operator/provider-accounts/chatgpt-codex/leases/grant";

const NativeCodexAuthSchema = Schema.Struct({
  auth_mode: Schema.Literals(["chatgpt", "chatgptAuthTokens"]),
  tokens: Schema.Struct({
    access_token: Schema.String,
    account_id: Schema.NullOr(Schema.String),
    id_token: Schema.String,
    refresh_token: Schema.String,
  }),
});
const decodeNativeCodexAuth = Schema.decodeUnknownSync(NativeCodexAuthSchema);

export type NativeCodexAccountStatus = Readonly<{
  authenticated: boolean;
  accountType: string | null;
  requiresOpenaiAuth: boolean;
  expectedIdentityMatched: boolean | null;
}>;

export type NativeCodexAuthMaterialization = Readonly<{
  codexHome: string;
  authJsonPath: string;
  account: NativeCodexAccountStatus;
}>;

export type NativeCodexAccountReader = (
  input: Readonly<{
    codexBinary: string;
    codexHome: string;
    timeoutMs: number;
  }>,
) => Promise<unknown>;

export class NativeCodexAuthError extends Error {
  readonly _tag = "NativeCodexAuthError";
  override readonly name = "NativeCodexAuthError";

  constructor(
    readonly reason:
      | "source_required"
      | "source_invalid"
      | "source_permissions"
      | "scratch_invalid"
      | "materialization_failed"
      | "account_validation_failed"
      | "account_unauthenticated"
      | "live_credential_import_not_armed"
      | "live_handoff_not_armed"
      | "secure_destination_unproven"
      | "credential_import_failed"
      | "lease_acquire_failed"
      | "grant_issue_failed",
    message: string,
  ) {
    super(message);
  }
}

const isContainedBy = (parent: string, child: string): boolean => {
  const nested = relative(parent, child);
  return nested !== "" && nested !== ".." && !nested.startsWith(`..${sep}`) && !isAbsolute(nested);
};

const isHttpsUrl = (value: string | undefined): boolean => {
  if (value === undefined) return false;
  try {
    return new URL(value).protocol === "https:";
  } catch {
    return false;
  }
};

const projectAccountStatus = (
  value: unknown,
  expectedAccountEmail?: string,
): NativeCodexAccountStatus => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new NativeCodexAuthError(
      "account_validation_failed",
      "Codex account/read returned an invalid response.",
    );
  }
  const response = value as Record<string, unknown>;
  const account = response.account;
  if (account === null || account === undefined) {
    return {
      authenticated: false,
      accountType: null,
      requiresOpenaiAuth: response.requiresOpenaiAuth === true,
      expectedIdentityMatched: expectedAccountEmail === undefined ? null : false,
    };
  }
  if (typeof account !== "object" || Array.isArray(account)) {
    throw new NativeCodexAuthError(
      "account_validation_failed",
      "Codex account/read returned an invalid account.",
    );
  }
  const accountType = (account as Record<string, unknown>).type;
  const accountEmail = (account as Record<string, unknown>).email;
  return {
    authenticated: true,
    accountType: typeof accountType === "string" ? accountType : "unknown",
    requiresOpenaiAuth: response.requiresOpenaiAuth === true,
    expectedIdentityMatched:
      expectedAccountEmail === undefined
        ? null
        : typeof accountEmail === "string" &&
          accountEmail.toLowerCase() === expectedAccountEmail.toLowerCase(),
  };
};

export const readNativeCodexAccount: NativeCodexAccountReader = async (input) => {
  const client = openCodexAppServerClient({
    binary: input.codexBinary,
    cwd: input.codexHome,
    env: {
      ...process.env,
      CODEX_HOME: input.codexHome,
    },
    requestTimeoutMs: input.timeoutMs,
  });
  try {
    await client.initialize();
    return await client.request("account/read", { refreshToken: false });
  } finally {
    client.close();
  }
};

const readAndValidateSource = (
  sourceAuthJson: string,
): Readonly<{
  bytes: Buffer;
}> => {
  if (sourceAuthJson.trim() === "") {
    throw new NativeCodexAuthError(
      "source_required",
      "An explicit --source-auth-json path is required.",
    );
  }
  const sourcePath = resolve(sourceAuthJson);
  if (!existsSync(sourcePath) || basename(sourcePath) !== "auth.json") {
    throw new NativeCodexAuthError(
      "source_invalid",
      "The native Codex source must be an existing auth.json file.",
    );
  }
  const status = lstatSync(sourcePath);
  if (!status.isFile() || status.size < 2 || status.size > NATIVE_CODEX_AUTH_MAX_BYTES) {
    throw new NativeCodexAuthError(
      "source_invalid",
      "The native Codex auth.json file is not a bounded regular file.",
    );
  }
  if ((status.mode & 0o077) !== 0) {
    throw new NativeCodexAuthError(
      "source_permissions",
      "The native Codex auth.json must not grant group or other access.",
    );
  }
  const bytes = readFileSync(sourcePath);
  try {
    const parsed = JSON.parse(bytes.toString("utf8")) as unknown;
    const decoded = decodeNativeCodexAuth(parsed);
    if (
      decoded.auth_mode.trim() === "" ||
      decoded.tokens.access_token.trim() === "" ||
      decoded.tokens.id_token.trim() === "" ||
      decoded.tokens.refresh_token.trim() === ""
    ) {
      throw new Error("empty credential field");
    }
  } catch {
    throw new NativeCodexAuthError(
      "source_invalid",
      "The native Codex auth.json does not match the supported app-server credential shape.",
    );
  }
  return { bytes };
};

export const materializeNativeCodexAuth = async (
  input: Readonly<{
    sourceAuthJson: string;
    scratchRoot: string;
    codexBinary?: string;
    timeoutMs?: number;
    accountReader?: NativeCodexAccountReader;
    expectedAccountEmail?: string;
  }>,
): Promise<NativeCodexAuthMaterialization> => {
  const sourcePath = resolve(input.sourceAuthJson);
  const scratchRoot = resolve(input.scratchRoot);
  const defaultCodexHome = resolve(homedir(), ".codex");
  if (
    sourcePath === scratchRoot ||
    isContainedBy(dirname(sourcePath), scratchRoot) ||
    scratchRoot === defaultCodexHome ||
    isContainedBy(defaultCodexHome, scratchRoot)
  ) {
    throw new NativeCodexAuthError(
      "scratch_invalid",
      "Scratch custody must not be inside the native credential source directory.",
    );
  }
  mkdirSync(scratchRoot, { recursive: true, mode: 0o700 });
  chmodSync(scratchRoot, 0o700);
  const source = readAndValidateSource(sourcePath);
  const codexHome = mkdtempSync(join(scratchRoot, "codex-home-"));
  chmodSync(codexHome, 0o700);
  const authJsonPath = join(codexHome, "auth.json");
  try {
    const descriptor = openSync(authJsonPath, "wx", 0o600);
    try {
      writeFileSync(descriptor, source.bytes);
    } finally {
      closeSync(descriptor);
    }
    chmodSync(authJsonPath, 0o600);
    writeFileSync(
      join(codexHome, NATIVE_CODEX_AUTH_MARKER),
      `${JSON.stringify({ schema: "openagents.native_codex_scratch.v1" })}\n`,
      { encoding: "utf8", mode: 0o600, flag: "wx" },
    );
  } catch (error) {
    rmSync(codexHome, { recursive: true, force: true });
    throw new NativeCodexAuthError(
      "materialization_failed",
      error instanceof Error ? error.message : "Native Codex materialization failed.",
    );
  }
  let account: NativeCodexAccountStatus;
  try {
    account = projectAccountStatus(
      await (input.accountReader ?? readNativeCodexAccount)({
        codexBinary: input.codexBinary ?? "codex",
        codexHome,
        timeoutMs: input.timeoutMs ?? NATIVE_CODEX_AUTH_DEFAULT_TIMEOUT_MS,
      }),
      input.expectedAccountEmail,
    );
  } catch (error) {
    rmSync(codexHome, { recursive: true, force: true });
    if (error instanceof NativeCodexAuthError) throw error;
    throw new NativeCodexAuthError(
      "account_validation_failed",
      "Codex account/read validation failed in isolated scratch custody.",
    );
  }
  if (!account.authenticated) {
    rmSync(codexHome, { recursive: true, force: true });
    throw new NativeCodexAuthError(
      "account_unauthenticated",
      "The isolated Codex app-server did not report an authenticated account.",
    );
  }
  if (account.expectedIdentityMatched === false) {
    rmSync(codexHome, { recursive: true, force: true });
    throw new NativeCodexAuthError(
      "account_validation_failed",
      "The native Codex identity does not match the selected owner identity.",
    );
  }
  return { codexHome, authJsonPath, account };
};

export const cleanupNativeCodexScratch = (
  input: Readonly<{
    codexHome: string;
    scratchRoot: string;
  }>,
): void => {
  const codexHome = resolve(input.codexHome);
  const scratchRoot = resolve(input.scratchRoot);
  if (
    !isContainedBy(scratchRoot, codexHome) ||
    !existsSync(join(codexHome, NATIVE_CODEX_AUTH_MARKER))
  ) {
    throw new NativeCodexAuthError(
      "scratch_invalid",
      "Refusing to clean an unmarked path outside the declared scratch root.",
    );
  }
  rmSync(codexHome, { recursive: true, force: true });
};

export type CodexLeaseGrantHandoff = Readonly<{
  leaseRef: string;
  providerAccountRef: string;
  authGrantRef: string;
  expiresAt: string;
  runnerSessionId: string;
}>;

export type NativeCodexCustodyImport = Readonly<{
  providerAccountRef: string;
  accountStatus: string;
  custodyStatus: "stored";
}>;

const nativeCodexOAuthPayload = (authJsonPath: string): Record<string, unknown> => {
  const decoded = decodeNativeCodexAuth(JSON.parse(readFileSync(authJsonPath, "utf8")) as unknown);
  return {
    type: "oauth",
    access: decoded.tokens.access_token,
    refresh: decoded.tokens.refresh_token,
    expires: 0,
    ...(decoded.tokens.account_id === null ? {} : { accountId: decoded.tokens.account_id }),
    idToken: decoded.tokens.id_token,
  };
};

export const importNativeCodexAuthForLease = async (
  input: Readonly<{
    baseUrl: string;
    operatorToken: string;
    ownerEmail: string;
    providerAccountRef?: string;
    authJsonPath: string;
    allowLiveCredentialImport: boolean;
    fetchImpl?: typeof fetch;
  }>,
): Promise<NativeCodexCustodyImport> => {
  if (!input.allowLiveCredentialImport) {
    throw new NativeCodexAuthError(
      "live_credential_import_not_armed",
      "Live credential import requires --allow-live-credential-import.",
    );
  }
  const endpoint = new URL(CODEX_NATIVE_AUTH_IMPORT_PATH, input.baseUrl);
  if (endpoint.protocol !== "https:" || input.operatorToken.trim() === "") {
    throw new NativeCodexAuthError(
      "secure_destination_unproven",
      "The existing credential custody destination requires HTTPS and an operator token.",
    );
  }
  const response = await (input.fetchImpl ?? fetch)(endpoint, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${input.operatorToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      email: input.ownerEmail,
      ...(input.providerAccountRef === undefined
        ? {}
        : { providerAccountRef: input.providerAccountRef }),
      auth: nativeCodexOAuthPayload(input.authJsonPath),
    }),
  });
  const payload = await response.json().catch((): unknown => null);
  if (!response.ok || payload === null || typeof payload !== "object" || Array.isArray(payload)) {
    throw new NativeCodexAuthError(
      "credential_import_failed",
      `Native Codex custody import failed with HTTP ${response.status}.`,
    );
  }
  const record = payload as Record<string, unknown>;
  if (
    record.ok !== true ||
    typeof record.providerAccountRef !== "string" ||
    typeof record.accountStatus !== "string" ||
    record.custodyStatus !== "stored"
  ) {
    throw new NativeCodexAuthError(
      "credential_import_failed",
      "Native Codex custody import response did not confirm the lease-bound destination.",
    );
  }
  return {
    providerAccountRef: record.providerAccountRef,
    accountStatus: record.accountStatus,
    custodyStatus: "stored",
  };
};

export type CodexLeaseAcquisition = Readonly<{
  leaseRef: string;
  providerAccountRef: string;
  expiresAt: string;
}>;

export const acquireCodexLease = async (
  input: Readonly<{
    baseUrl: string;
    operatorToken: string;
    ownerEmail: string;
    providerAccountRef: string;
    runnerSessionId: string;
    fetchImpl?: typeof fetch;
  }>,
): Promise<CodexLeaseAcquisition> => {
  const endpoint = new URL(CODEX_LEASE_ACQUIRE_PATH, input.baseUrl);
  if (endpoint.protocol !== "https:" || input.operatorToken.trim() === "") {
    throw new NativeCodexAuthError(
      "secure_destination_unproven",
      "The existing lease destination requires HTTPS and an operator token.",
    );
  }
  const response = await (input.fetchImpl ?? fetch)(endpoint, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${input.operatorToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      email: input.ownerEmail,
      providerAccountRef: input.providerAccountRef,
      requiredProvider: "chatgpt_codex",
      requestedAction: "agent_computer.codex_turn",
      runId: input.runnerSessionId,
    }),
  });
  const payload = await response.json().catch((): unknown => null);
  if (!response.ok || payload === null || typeof payload !== "object" || Array.isArray(payload)) {
    throw new NativeCodexAuthError(
      "lease_acquire_failed",
      `Codex lease acquisition failed with HTTP ${response.status}.`,
    );
  }
  const record = payload as Record<string, unknown>;
  if (
    typeof record.leaseRef !== "string" ||
    record.providerAccountRef !== input.providerAccountRef ||
    typeof record.expiresAt !== "string" ||
    record.status !== "active"
  ) {
    throw new NativeCodexAuthError(
      "lease_acquire_failed",
      "Codex lease acquisition did not bind the imported provider account.",
    );
  }
  return {
    leaseRef: record.leaseRef,
    providerAccountRef: input.providerAccountRef,
    expiresAt: record.expiresAt,
  };
};

export const issueExistingCodexLeaseGrant = async (
  input: Readonly<{
    baseUrl: string;
    operatorToken: string;
    ownerEmail: string;
    leaseRef: string;
    runnerSessionId: string;
    workroomId: string;
    allowLiveHandoff: boolean;
    fetchImpl?: typeof fetch;
  }>,
): Promise<CodexLeaseGrantHandoff> => {
  if (!input.allowLiveHandoff) {
    throw new NativeCodexAuthError(
      "live_handoff_not_armed",
      "Live grant issuance requires --allow-live-handoff.",
    );
  }
  const endpoint = new URL(CODEX_LEASE_GRANT_PATH, input.baseUrl);
  if (endpoint.protocol !== "https:" || input.operatorToken.trim() === "") {
    throw new NativeCodexAuthError(
      "secure_destination_unproven",
      "The existing grant destination requires HTTPS and an operator token.",
    );
  }
  const response = await (input.fetchImpl ?? fetch)(endpoint, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${input.operatorToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      email: input.ownerEmail,
      leaseRef: input.leaseRef,
      runnerSessionId: input.runnerSessionId,
      workroomId: input.workroomId,
      requestedAction: "agent_computer.codex_turn",
    }),
  });
  const payload = await response.json().catch((): unknown => null);
  if (!response.ok || payload === null || typeof payload !== "object" || Array.isArray(payload)) {
    throw new NativeCodexAuthError(
      "grant_issue_failed",
      `Existing Codex lease grant issuance failed with HTTP ${response.status}.`,
    );
  }
  const record = payload as Record<string, unknown>;
  const grant = record.grant;
  if (
    typeof record.leaseRef !== "string" ||
    typeof record.providerAccountRef !== "string" ||
    grant === null ||
    typeof grant !== "object"
  ) {
    throw new NativeCodexAuthError(
      "grant_issue_failed",
      "Existing Codex lease grant response was invalid.",
    );
  }
  const grantRecord = grant as Record<string, unknown>;
  if (
    typeof grantRecord.grantRef !== "string" ||
    typeof grantRecord.expiresAt !== "string" ||
    grantRecord.runnerSessionId !== input.runnerSessionId
  ) {
    throw new NativeCodexAuthError(
      "grant_issue_failed",
      "Existing Codex lease grant response did not bind the requested turn session.",
    );
  }
  return {
    leaseRef: record.leaseRef,
    providerAccountRef: record.providerAccountRef,
    authGrantRef: grantRecord.grantRef,
    expiresAt: grantRecord.expiresAt,
    runnerSessionId: input.runnerSessionId,
  };
};

type ParsedCli = Readonly<{
  command: string;
  values: ReadonlyMap<string, string>;
  flags: ReadonlySet<string>;
}>;

const parseCli = (argv: ReadonlyArray<string>): ParsedCli => {
  const command = argv[0] ?? "status";
  const values = new Map<string, string>();
  const flags = new Set<string>();
  for (let index = 1; index < argv.length; index += 1) {
    const token = argv[index]!;
    if (!token.startsWith("--"))
      throw new NativeCodexAuthError("source_invalid", `Unexpected argument: ${token}`);
    const next = argv[index + 1];
    if (next === undefined || next.startsWith("--")) {
      flags.add(token);
      continue;
    }
    values.set(token, next);
    index += 1;
  }
  return { command, values, flags };
};

const requiredValue = (parsed: ParsedCli, name: string): string => {
  const value = parsed.values.get(name);
  if (value === undefined || value.trim() === "")
    throw new NativeCodexAuthError("source_required", `${name} is required.`);
  return value;
};

export const runNativeCodexAuthCli = async (
  argv: ReadonlyArray<string>,
  env: NodeJS.ProcessEnv = process.env,
  dependencies: Readonly<{
    accountReader?: NativeCodexAccountReader;
    fetchImpl?: typeof fetch;
  }> = {},
): Promise<Record<string, unknown>> => {
  const parsed = parseCli(argv);
  if (parsed.command === "cleanup") {
    cleanupNativeCodexScratch({
      codexHome: requiredValue(parsed, "--codex-home"),
      scratchRoot: requiredValue(parsed, "--scratch-root"),
    });
    return { ok: true, command: "cleanup", cleaned: true };
  }
  if (!["status", "dry-run", "materialize", "handoff"].includes(parsed.command)) {
    throw new NativeCodexAuthError("source_invalid", `Unknown command: ${parsed.command}`);
  }
  const scratchRoot = requiredValue(parsed, "--scratch-root");
  const materialized = await materializeNativeCodexAuth({
    sourceAuthJson: requiredValue(parsed, "--source-auth-json"),
    scratchRoot,
    codexBinary: parsed.values.get("--codex-binary"),
    accountReader: dependencies.accountReader,
    expectedAccountEmail: parsed.values.get("--owner-email"),
  });
  const publicStatus = {
    ok: true,
    command: parsed.command,
    authenticated: materialized.account.authenticated,
    accountType: materialized.account.accountType,
    requiresOpenaiAuth: materialized.account.requiresOpenaiAuth,
    expectedIdentityMatched: materialized.account.expectedIdentityMatched,
    sourceMutated: false,
    defaultCodexHomeMutated: false,
  };
  if (parsed.command === "status" || parsed.command === "dry-run") {
    cleanupNativeCodexScratch({ codexHome: materialized.codexHome, scratchRoot });
    const baseUrl = parsed.values.get("--base-url");
    const handoffMissing =
      parsed.command === "dry-run"
        ? [
            ...(isHttpsUrl(baseUrl) ? [] : ["https_base_url"]),
            ...(parsed.values.has("--owner-email") ? [] : ["owner_email"]),
            ...(parsed.values.has("--runner-session-id") ? [] : ["runner_session_id"]),
            ...(parsed.values.has("--workroom-id") ? [] : ["workroom_id"]),
            ...(env.OPENAGENTS_OPERATOR_ADMIN_TOKEN?.trim() ? [] : ["operator_token"]),
          ]
        : [];
    return {
      ...publicStatus,
      retainedScratch: false,
      liveHandoffAttempted: false,
      ...(parsed.command === "dry-run"
        ? {
            handoffReady: handoffMissing.length === 0,
            handoffMissing,
          }
        : {}),
    };
  }
  if (parsed.command === "materialize") {
    return {
      ...publicStatus,
      retainedScratch: true,
      codexHome: materialized.codexHome,
      authMode: "mode-0600-auth-in-mode-0700-codex-home",
    };
  }
  if (!parsed.flags.has("--allow-live-credential-import")) {
    cleanupNativeCodexScratch({ codexHome: materialized.codexHome, scratchRoot });
    throw new NativeCodexAuthError(
      "live_credential_import_not_armed",
      "Live credential import requires --allow-live-credential-import.",
    );
  }
  if (!parsed.flags.has("--allow-live-handoff")) {
    cleanupNativeCodexScratch({ codexHome: materialized.codexHome, scratchRoot });
    throw new NativeCodexAuthError(
      "live_handoff_not_armed",
      "Live grant issuance requires --allow-live-handoff.",
    );
  }
  try {
    const custodyImport = await importNativeCodexAuthForLease({
      baseUrl: requiredValue(parsed, "--base-url"),
      operatorToken: env.OPENAGENTS_OPERATOR_ADMIN_TOKEN ?? "",
      ownerEmail: requiredValue(parsed, "--owner-email"),
      providerAccountRef: parsed.values.get("--provider-account-ref"),
      authJsonPath: materialized.authJsonPath,
      allowLiveCredentialImport: true,
      fetchImpl: dependencies.fetchImpl,
    });
    const lease = await acquireCodexLease({
      baseUrl: requiredValue(parsed, "--base-url"),
      operatorToken: env.OPENAGENTS_OPERATOR_ADMIN_TOKEN ?? "",
      ownerEmail: requiredValue(parsed, "--owner-email"),
      providerAccountRef: custodyImport.providerAccountRef,
      runnerSessionId: requiredValue(parsed, "--runner-session-id"),
      fetchImpl: dependencies.fetchImpl,
    });
    const handoff = await issueExistingCodexLeaseGrant({
      baseUrl: requiredValue(parsed, "--base-url"),
      operatorToken: env.OPENAGENTS_OPERATOR_ADMIN_TOKEN ?? "",
      ownerEmail: requiredValue(parsed, "--owner-email"),
      leaseRef: lease.leaseRef,
      runnerSessionId: requiredValue(parsed, "--runner-session-id"),
      workroomId: requiredValue(parsed, "--workroom-id"),
      allowLiveHandoff: parsed.flags.has("--allow-live-handoff"),
      fetchImpl: dependencies.fetchImpl,
    });
    if (custodyImport.providerAccountRef !== handoff.providerAccountRef) {
      throw new NativeCodexAuthError(
        "grant_issue_failed",
        "The issued grant did not use the lease account that received native Codex custody.",
      );
    }
    cleanupNativeCodexScratch({ codexHome: materialized.codexHome, scratchRoot });
    return {
      ...publicStatus,
      retainedScratch: false,
      liveHandoffAttempted: true,
      custodyImport,
      lease,
      handoff,
      agentComputerProviderAuth: {
        providerAccountRef: handoff.providerAccountRef,
        authGrantRef: handoff.authGrantRef,
      },
    };
  } catch (error) {
    cleanupNativeCodexScratch({ codexHome: materialized.codexHome, scratchRoot });
    throw error;
  }
};

const isCli =
  process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isCli) {
  runNativeCodexAuthCli(process.argv.slice(2))
    .then((result) => process.stdout.write(`${JSON.stringify(result)}\n`))
    .catch((error) => {
      const result =
        error instanceof NativeCodexAuthError
          ? { ok: false, reason: error.reason, message: error.message }
          : { ok: false, reason: "internal", message: "Native Codex auth command failed." };
      process.stdout.write(`${JSON.stringify(result)}\n`);
      process.exitCode = 1;
    });
}
