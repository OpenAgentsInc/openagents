/**
 * @openagentsinc/omega-effectd
 *
 * Supervised Full Auto engine for Omega. Desktop is the first workspace
 * consumer. Omega pins a packed artifact digest (never a relative monorepo
 * path).
 */

export {
  FULL_AUTO_CONTROL_FILE,
  FULL_AUTO_DIR_NAME,
  FULL_AUTO_REGISTRY_FILE,
  FULL_AUTO_RUNS_FILE,
  FULL_AUTO_RUN_REPORTS_FILE,
  resolveFullAutoControlPath,
  resolveFullAutoDir,
  resolveFullAutoRegistryPath,
  resolveFullAutoRunReportsPath,
  resolveFullAutoRunsPath,
  type OmegaEffectdPaths,
} from "./paths.ts";

export {
  FULL_AUTO_RUN_ACTIVE_LIMIT,
  FULL_AUTO_RUN_LEGAL_TRANSITIONS,
  FullAutoRunStateSchema,
} from "./engine/full-auto-run-registry.ts";

export { FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS } from "./engine/full-auto-reconcile.ts";

export { FULL_AUTO_DEFAULT_LANE } from "./engine/full-auto-lane.ts";

export { FULL_AUTO_RUN_RECEIPT_SCHEMA } from "./engine/full-auto-run-report.ts";

export {
  createOmegaEffectdService,
  type OmegaEffectdService,
  type OmegaEffectdServiceOptions,
} from "./service.ts";

export {
  OMEGA_EFFECTD_MAX_FRAME_BYTES,
  OMEGA_EFFECTD_MAX_HOST_REQUESTS,
  OMEGA_EFFECTD_PROTOCOL_SCHEMA,
  OMEGA_EFFECTD_PROTOCOL_VERSION,
  OMEGA_EFFECTD_SERVICE_VERSION,
  type OmegaEffectdHostMethod,
  type OmegaEffectdHostRequest,
  type OmegaEffectdHostResponse,
  type OmegaEffectdRunSnapshot,
} from "./protocol/framed.ts";

export {
  OmegaEffectdHostBridge,
  OmegaEffectdHostBridgeError,
  type HostReplyDisposition,
  type OmegaEffectdHostBridgeOptions,
  type OmegaEffectdHostFrameEmitter,
} from "./protocol/host-bridge.ts";

export {
  createOmegaEffectdFramedServer,
  type OmegaEffectdFramedServer,
} from "./protocol/server.ts";
