/**
 * QR intake for destination entry (issue #9317 §8): scanning is offered
 * where a camera exists and every scanned payload routes through the same
 * shared parser as the address and invoice fields — there is no separate
 * QR interpretation path.
 *
 * The camera itself (getUserMedia, frame decoding) is host work; this file
 * defines the capability port the shell probes and the single entry point
 * a scan result takes into the parser. There is no clipboard read on
 * destination fields (issue #9317, out of scope) and none here.
 */
import { Effect } from "effect";

import type { DestinationParseResult } from "./model.js";
import { parseDestination, type DestinationParseContext } from "./parse.js";

export type QrAvailability = "unknown" | "unavailable" | "available";

/**
 * Host-implemented camera capability. `probe` must be side-effect free
 * beyond enumeration (no permission prompt): the scan affordance renders
 * only where a camera exists, mirroring the teardown's camera-probe gate.
 */
export interface QrScanCapability {
  readonly probe: Effect.Effect<QrAvailability>;
}

export const qrUnavailable: QrScanCapability = {
  probe: Effect.succeed("unavailable" as const),
};

/**
 * Route a scanned payload through the shared parser. QR payloads often use
 * the uppercase bech32 alternate form; the shared parser normalises case,
 * so a scan is exactly a paste.
 */
export const acceptScannedText = (
  text: string,
  context: DestinationParseContext,
): DestinationParseResult => parseDestination(text, context);
