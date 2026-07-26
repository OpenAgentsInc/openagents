/**
 * Bind the omega#47 Full Auto detail projection to the host this device is
 * actually paired to (omega#49).
 *
 * `readIssue31FullAutoProjection` already refuses to render a detail payload
 * that is missing, unreadable, or bound to another snapshot. Nothing was ever
 * calling it with a real payload, so the Workroom's Full Auto section reported
 * `no_host_projection` on every device — including a paired one holding a
 * confirmed grant. That is a wiring gap reported as a pairing fact, which is
 * exactly the class of defect this contract exists to prevent.
 *
 * This module closes the gap and keeps the distinction the contract draws:
 *
 * - the *device's signed grant* chooses the host, never the payload. An adjunct
 *   that names a host this device never paired with cannot select itself into
 *   view by asserting its own `hostRef`;
 * - the `snapshotRef` the detail is checked against comes from the host's own
 *   `host.v1` snapshot record, not from the detail record, so
 *   `isIssue31FullAutoAdjunctBoundTo` compares two independent statements
 *   rather than comparing a payload to itself;
 * - an unpaired device reads `no_host_projection`, a paired device holding a
 *   malformed detail reads `host_projection_unreadable`, and a paired device
 *   holding a detail for another host or another snapshot reads
 *   `snapshot_mismatch`. None of the three is allowed to wear another's copy.
 *
 * Wire note: `Issue31PrivateRecord` once admitted five schemas and neither
 * `host.v1` nor `fullauto.v1` was one of them, so the client dropped both
 * before any reader saw them. Both are admitted now, delivery-bound, and proven
 * across a real relay by `issue31-full-auto-delivery.test.ts`. This reader is
 * still deliberately written against the confirmed-event shape rather than that
 * union — it reads whatever owner-private record the client hands it and
 * refuses everything it cannot bind.
 */
import {
  ISSUE31_PAIRING_SCHEMA,
  foldIssue31Grant,
  type Issue31GrantState,
  type Issue31PairingEvent,
  type Issue31PairingRecord,
} from "@openagentsinc/sarah/issue31-nostr";
import {
  ISSUE31_FULL_AUTO_ADJUNCT_SCHEMA,
  ISSUE31_HOST_ADJUNCT_SCHEMA,
  decodeIssue31HostAdjunct,
  type Issue31HostAdjunct,
} from "@openagentsinc/sarah/issue31-workroom";

import {
  ISSUE31_FULL_AUTO_READ_MODEL_SCHEMA,
  readIssue31FullAutoProjection,
  type Issue31FullAutoReadModel,
  type Issue31FullAutoUnavailableReason,
} from "./issue31-full-auto-read-model.ts";

/**
 * The part of a confirmed event this reader is allowed to look at.
 *
 * `privateRecord` is `unknown` on purpose: the reader must not assume the
 * record union already carries the adjunct schemas, and it must not trust the
 * record's own claims before the grant has chosen a host.
 */
export interface Issue31FullAutoSourceEvent {
  readonly canonicalRecordId: string;
  readonly event: { readonly created_at: number };
  readonly privateRecord: unknown;
}

/** The part of an `Issue31NostrClientSnapshot` this reader is allowed to read. */
export interface Issue31FullAutoSourceSnapshot {
  readonly devicePublicKeyHex: string | null;
  readonly admittedHostPublicKeys: ReadonlyArray<string>;
  readonly selectedHostPublicKeys: ReadonlyArray<string>;
  readonly confirmedEvents: ReadonlyArray<Issue31FullAutoSourceEvent>;
}

/** State the Full Auto section shows when it may not show runs. */
export const issue31FullAutoProjectionUnavailable = (
  reason: Issue31FullAutoUnavailableReason,
): Issue31FullAutoReadModel => ({
  schema: ISSUE31_FULL_AUTO_READ_MODEL_SCHEMA,
  state: "unavailable",
  reason,
});

const schemaOf = (record: unknown): string | null => {
  if (record === null || typeof record !== "object" || Array.isArray(record)) return null;
  const value = (record as Readonly<Record<string, unknown>>)["schema"];
  return typeof value === "string" ? value : null;
};

/**
 * Confirmed records carrying `schema`, newest first.
 *
 * Ordering is by the relay-confirmed event time with an identifier tie-break so
 * two devices reading the same relay pick the same record. "Newest" is never
 * derived from a field inside the payload, because an undecodable payload has
 * no readable fields at all and must still be orderable.
 */
const recordsWithSchema = (
  snapshot: Issue31FullAutoSourceSnapshot,
  schema: string,
): ReadonlyArray<Issue31FullAutoSourceEvent> =>
  snapshot.confirmedEvents
    .filter((event) => schemaOf(event.privateRecord) === schema)
    .slice()
    .sort(
      (left, right) =>
        right.event.created_at - left.event.created_at ||
        right.canonicalRecordId.localeCompare(left.canonicalRecordId),
    );

/**
 * The grant that entitles this device to read this host, or `null`.
 *
 * The admission rules match the ones the source projection already applies: the
 * record's host key must be both admitted by the build and selected by the
 * owner, and the record must name this exact device. A grant folded from a
 * forked chain is discarded rather than repaired.
 */
export const activeIssue31GrantForDevice = (
  snapshot: Issue31FullAutoSourceSnapshot,
  nowUnixSeconds: number,
): Issue31GrantState | null => {
  const devicePublicKeyHex = snapshot.devicePublicKeyHex;
  if (devicePublicKeyHex === null) return null;
  const admitted = new Set(snapshot.admittedHostPublicKeys);
  const selected = new Set(snapshot.selectedHostPublicKeys);
  const pairingEvents = snapshot.confirmedEvents.flatMap(
    (event): ReadonlyArray<Issue31PairingEvent> => {
      if (schemaOf(event.privateRecord) !== ISSUE31_PAIRING_SCHEMA) return [];
      const record = event.privateRecord as Issue31PairingRecord;
      return admitted.has(record.hostPublicKeyHex) &&
        selected.has(record.hostPublicKeyHex) &&
        record.devicePublicKeyHex === devicePublicKeyHex
        ? [{ eventId: event.canonicalRecordId, record }]
        : [];
    },
  );
  const grantRefs = new Set(
    pairingEvents.flatMap(({ record }) =>
      record.recordType === "scoped_grant" ||
      record.recordType === "grant_renewal" ||
      record.recordType === "grant_revocation"
        ? [record.grantRef]
        : [],
    ),
  );
  return (
    [...grantRefs]
      .flatMap((grantRef) => {
        try {
          const grant = foldIssue31Grant(pairingEvents, grantRef);
          return grant === null ? [] : [grant];
        } catch {
          // A forked grant chain is not a grant. Reading one would be reading
          // an unresolved conflict as an entitlement.
          return [];
        }
      })
      .filter(
        (grant) =>
          grant.status === "active" &&
          grant.expiresAt !== null &&
          grant.expiresAt > nowUnixSeconds &&
          grant.devicePublicKeyHex === devicePublicKeyHex,
      )
      .sort(
        (left, right) =>
          right.issuedAt - left.issuedAt ||
          right.sourceEventId.localeCompare(left.sourceEventId),
      )[0] ?? null
  );
};

/**
 * Which `host.v1` snapshot this device is entitled to read, or why none.
 *
 * The four states are kept apart because they are four different facts and only
 * one of them is "no Omega host here". `unpaired` is the absence of a grant;
 * `absent` is a live grant whose host has published nothing; `unreadable` is a
 * host that published something this device refuses to read; `bound` is the
 * snapshot itself.
 */
export type Issue31HostAdjunctBinding =
  | { readonly state: "bound"; readonly host: Issue31HostAdjunct }
  | { readonly state: "unpaired" }
  | { readonly state: "absent" }
  | { readonly state: "unreadable" };

/**
 * The `host.v1` snapshot this device may bind detail payloads to.
 *
 * Returns the host adjunct itself when one is readable and belongs to the
 * granted host, and otherwise says which kind of absence it is: a host record
 * that failed the contract is `unreadable`, and no such record at all — or one
 * that belongs to some other host — is `absent`.
 */
const hostBindingFor = (
  snapshot: Issue31FullAutoSourceSnapshot,
  grant: Issue31GrantState,
):
  | { readonly state: "bound"; readonly host: Issue31HostAdjunct }
  | { readonly state: "absent" }
  | { readonly state: "unreadable" } => {
  let unreadable = false;
  for (const event of recordsWithSchema(snapshot, ISSUE31_HOST_ADJUNCT_SCHEMA)) {
    let host: Issue31HostAdjunct;
    try {
      host = decodeIssue31HostAdjunct(event.privateRecord);
    } catch {
      unreadable = true;
      continue;
    }
    // The grant names the host. A snapshot record naming a different host is
    // another machine's state and never this device's Full Auto view.
    if (host.hostRef === grant.hostRef) return { state: "bound", host };
  }
  return unreadable ? { state: "unreadable" } : { state: "absent" };
};

/**
 * The host snapshot this device is entitled to read, from a confirmed snapshot.
 *
 * This is the single binding both Workroom surfaces use: the capability rows
 * (`projectIssue31WorkroomReadModel`'s `hostAdjunct`) and the Full Auto section
 * below them. omega#97's host half found the mirror-image defect on the desktop
 * — one reading of a daemon inlined in a view, so a second surface would have
 * read a second reading of one machine. A row that said one thing about a host
 * above a section that said another would be the same defect on the phone.
 *
 * Total by construction: an unreadable host record is a named state, never a
 * throw into the render.
 */
export const issue31HostAdjunctForDevice = (
  snapshot: Issue31FullAutoSourceSnapshot,
  nowUnixSeconds: number,
): Issue31HostAdjunctBinding => {
  const grant = activeIssue31GrantForDevice(snapshot, nowUnixSeconds);
  // No grant is the one case that really is "not paired to an Omega host yet".
  if (grant === null) return { state: "unpaired" };
  return hostBindingFor(snapshot, grant);
};

/**
 * Project the Full Auto section from a confirmed Nostr snapshot.
 *
 * Total by construction: every refusal path returns an explicit unavailable
 * reason rather than throwing into the render.
 */
export const issue31FullAutoProjectionFromSnapshot = (
  snapshot: Issue31FullAutoSourceSnapshot,
  nowUnixSeconds: number,
): Issue31FullAutoReadModel => {
  const binding = issue31HostAdjunctForDevice(snapshot, nowUnixSeconds);
  if (binding.state === "unpaired") {
    return issue31FullAutoProjectionUnavailable("no_host_projection");
  }
  if (binding.state === "unreadable") {
    return issue31FullAutoProjectionUnavailable("host_projection_unreadable");
  }
  if (binding.state === "absent") {
    // Paired, and the host has published nothing for this grant. Saying "not
    // paired" here would contradict the grant this very function just read.
    return issue31FullAutoProjectionUnavailable("no_host_snapshot");
  }

  const detail = recordsWithSchema(snapshot, ISSUE31_FULL_AUTO_ADJUNCT_SCHEMA)[0];
  // The host is paired and has published a snapshot, but has said nothing about
  // Full Auto. Silence is reported as absence, never as an empty run list.
  if (detail === undefined) return issue31FullAutoProjectionUnavailable("no_full_auto_detail");

  // Only here does the payload get a say, and only against the host's own
  // independently published snapshot reference.
  return readIssue31FullAutoProjection(detail.privateRecord, {
    hostRef: binding.host.hostRef,
    snapshotRef: binding.host.snapshotRef,
  });
};
