/**
 * Host-owned ambient-context builder for the on-device Apple FM prompt.
 *
 * The Electron main process holds every fact the local assistant needs to answer
 * "what do you know about me" truthfully — the active working directory, the OS,
 * the running app name, the current date, and the sovereign PUBLIC identity. This
 * pure module maps those raw host inputs into the `AppleFmEnvironmentContext` the
 * prompt renders, WITHOUT importing Electron, so the mapping is unit-testable.
 *
 * The date is derived from an INJECTED clock (`now: Date`), never an unguarded
 * `new Date()`, so tests are deterministic. PUBLIC identity only: the caller
 * passes the sovereign `npub`; the prompt renderer additionally tripwires it, so
 * a mnemonic/`nsec`/seed/private key can never reach the model.
 */
import type { AppleFmEnvironmentContext } from "./apple-fm-prompt.ts";

/** Raw host facts, resolved by main at turn time and fed into the pure builder. */
export interface AppleFmEnvironmentInputs {
  /** The current instant from an injected clock (never an unguarded `new Date()`). */
  readonly now: Date;
  /** `process.platform` (e.g. "darwin"), mapped to a friendly OS label. */
  readonly platform: string;
  /** The running application name (e.g. "OpenAgents Dev"). */
  readonly appName?: string | null;
  /** The active absolute working directory. */
  readonly workingDirectory?: string | null;
  /** The sovereign PUBLIC `npub` (never an nsec/mnemonic/seed/private key). */
  readonly identityNpub?: string | null;
  /** True when this is the human owner's own device (Desktop always is). */
  readonly isOwnerDevice?: boolean;
}

const WEEKDAYS = [
  "Sunday",
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
] as const;
const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
] as const;

/** Map a Node `process.platform` value to a friendly OS label. */
export const platformLabel = (platform: string): string =>
  platform === "darwin"
    ? "macOS"
    : platform === "win32"
      ? "Windows"
      : platform === "linux"
        ? "Linux"
        : platform;

/**
 * Format an injected instant as a stable human date (UTC, no `Intl` locale data),
 * e.g. "Monday, July 20, 2026". Deterministic for a given `Date`.
 */
export const humanDateFrom = (now: Date): string =>
  `${WEEKDAYS[now.getUTCDay()]}, ${MONTHS[now.getUTCMonth()]} ${now.getUTCDate()}, ${now.getUTCFullYear()}`;

/**
 * Build the host-owned ambient context from raw inputs. Every field is optional
 * and fail-soft; a blank/absent value omits its line downstream. The date always
 * comes from the injected `now` so tests never depend on wall-clock time.
 */
export const buildAppleFmEnvironmentContext = (
  inputs: AppleFmEnvironmentInputs,
): AppleFmEnvironmentContext => {
  const context: {
    -readonly [K in keyof AppleFmEnvironmentContext]: AppleFmEnvironmentContext[K];
  } = {
    nowIso: inputs.now.toISOString(),
    humanDate: humanDateFrom(inputs.now),
    platform: platformLabel(inputs.platform),
  };
  if (typeof inputs.appName === "string" && inputs.appName.trim() !== "")
    context.appName = inputs.appName.trim();
  if (typeof inputs.workingDirectory === "string" && inputs.workingDirectory.trim() !== "")
    context.workingDirectory = inputs.workingDirectory.trim();
  if (typeof inputs.identityNpub === "string" && inputs.identityNpub.trim() !== "")
    context.identityNpub = inputs.identityNpub.trim();
  if (inputs.isOwnerDevice === true) context.isOwnerDevice = true;
  return context;
};
