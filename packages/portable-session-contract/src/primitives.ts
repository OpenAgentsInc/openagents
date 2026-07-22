import { Schema as S } from "effect";

export const PortableRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(/^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/),
);
export type PortableRef = typeof PortableRef.Type;

export const Sha256Digest = S.String.check(S.isPattern(/^sha256:[a-f0-9]{64}$/));
export type Sha256Digest = typeof Sha256Digest.Type;

/**
 * ENV-1 vocabulary (docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md,
 * "Environment and endpoint vocabulary (ENV-1)"): the owner-scoped identity of
 * one ExecutionEnvironment — a local Pylon, an owner-managed remote
 * Pylon/oa-node, an OpenAgents Agent Computer, or an audited managed-provider
 * workspace. The identity binds to the owner scope that enrolled the
 * environment and to its enrollment/health receipts, never to a bare
 * hostname, address, or process. How a client currently reaches the
 * environment (an AccessEndpoint, possibly hinted by an AdvertisedEndpoint)
 * is a connection-layer fact that never enters this identity, and switching
 * AccessEndpoint or KnownEnvironment entry must never create, transfer, or
 * fence execution authority — only the attachment-generation contract does.
 * Wire shape is exactly `PortableRef`; this alias adds vocabulary, not a
 * serialization change.
 */
export const ExecutionEnvironmentRef = PortableRef;
export type ExecutionEnvironmentRef = typeof ExecutionEnvironmentRef.Type;

export const PortableTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/),
);

export const PortableTargetClass = S.Literals([
  "owner_local",
  "owner_managed",
  "openagents_managed",
  "managed_provider",
]);
export type PortableTargetClass = typeof PortableTargetClass.Type;
