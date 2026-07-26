/**
 * The one projection the Workroom screen runs on every confirmed snapshot
 * (omega#97).
 *
 * ## Why this module exists
 *
 * The mobile read models and the renderer were both built and tested, and the
 * three `omega_host_adjunct` capability rows still read
 * `reason.issue31.source_not_connected` on a paired device holding a live host
 * snapshot — because nothing joined them. `projectIssue31WorkroomReadModel`
 * accepted a `hostAdjunct` and the screen passed it zero times.
 *
 * Composing that join inside `home-screen.tsx` would have left it untestable
 * except by a test that re-implements the screen, which proves the test. So the
 * composition lives here, the screen calls it, and the proofs call the same
 * function the screen calls.
 *
 * ## Two properties this file exists to hold
 *
 * **One host binding, two surfaces.** The capability rows and the Full Auto
 * section below them describe the same machine, so they read one binding rather
 * than each choosing a host record for itself. omega#97's host half found the
 * mirror image on the desktop: the daemon poll was inlined in a GPUI view, so a
 * second surface would have read a second reading of one daemon.
 *
 * **Fail visible, not fail closed.** A host record this device refuses to read
 * is a fact about one host, reported on the rows that expect that host. It may
 * not take the owner-private transcript, memory, read state, or either
 * community room down with it. A single malformed record blanking every surface
 * on the device is a defect this contract already paid for once.
 */
import type { Issue31FullAutoReadModel } from "./issue31-full-auto-read-model.ts";
import {
  issue31FullAutoProjectionFromSnapshot,
  issue31FullAutoProjectionUnavailable,
  issue31HostAdjunctForDevice,
  type Issue31HostAdjunctBinding,
} from "./issue31-full-auto-projection-source.ts";
import type { Issue31NostrClientSnapshot } from "./issue31-nostr-client.ts";
import { issue31SourceSnapshotsFromNostr } from "./issue31-nostr-read-model.ts";
import { projectIssue31OwnerPrivateReadModel } from "./issue31-owner-private-read-model.ts";
import {
  projectIssue31WorkroomReadModel,
  unavailableIssue31NostrWorkroomReadModel,
  type Issue31HostAdjunctGap,
  type Issue31WorkroomReadModel,
} from "./issue31-workroom-read-model.ts";

export interface Issue31WorkroomProjection {
  readonly workroom: Issue31WorkroomReadModel;
  readonly fullAuto: Issue31FullAutoReadModel;
  /** Which host record — if any — both halves above were computed from. */
  readonly hostBinding: Issue31HostAdjunctBinding;
}

/**
 * Which host gap the capability rows should state, given the binding.
 *
 * An unpaired device states none: `source_not_connected` is the true reason
 * there, and naming a host gap would invent a host that was never claimed. The
 * other two are facts about a host this device really is paired to.
 */
const hostGapFor = (binding: Issue31HostAdjunctBinding): Issue31HostAdjunctGap | undefined =>
  binding.state === "absent"
    ? "no_host_snapshot"
    : binding.state === "unreadable"
      ? "host_projection_unreadable"
      : undefined;

export const projectIssue31Workroom = (
  snapshot: Issue31NostrClientSnapshot,
  nowUnixSeconds: number,
): Issue31WorkroomProjection => {
  // Bound first, and in its own `try`: a host defect must not get a vote on
  // whether the owner-private room renders at all.
  const hostBinding = ((): Issue31HostAdjunctBinding => {
    try {
      return issue31HostAdjunctForDevice(snapshot, nowUnixSeconds);
    } catch {
      return { state: "unreadable" };
    }
  })();
  const hostGap = hostGapFor(hostBinding);
  const projectedAt = new Date(nowUnixSeconds * 1_000).toISOString();

  let workroom: Issue31WorkroomReadModel;
  try {
    // The owner-private projection runs first because the capability rows state
    // what this device *read*, not what it saw addressed to someone else.
    // Computing the rows from wire presence alone is how omega#49 came to show
    // `device_projection_missing:memory` above a room that was rendering the
    // projected engram (openagents `ba28b6fa24` lineage).
    const ownerPrivate = projectIssue31OwnerPrivateReadModel(snapshot, {
      nowUnixSeconds,
      transcriptLimit: 200,
    });
    workroom = projectIssue31WorkroomReadModel({
      projectedAt,
      sources: issue31SourceSnapshotsFromNostr(snapshot, nowUnixSeconds, ownerPrivate.projected),
      ownerPrivate,
      ...(hostBinding.state === "bound" ? { hostAdjunct: hostBinding.host } : {}),
      ...(hostGap === undefined ? {} : { hostAdjunctGap: hostGap }),
    });
  } catch {
    workroom = unavailableIssue31NostrWorkroomReadModel(
      projectedAt,
      "reason.issue31.nostr_projection_failed",
    );
  }

  // Full Auto is bound to the host the device's signed grant names, and to that
  // host's own snapshot reference — never to the detail payload's own claims.
  // Its absence, unreadability, and mismatch stay three distinct states rather
  // than collapsing into "not paired" (omega#49).
  let fullAuto: Issue31FullAutoReadModel;
  try {
    fullAuto = issue31FullAutoProjectionFromSnapshot(snapshot, nowUnixSeconds);
  } catch {
    fullAuto = issue31FullAutoProjectionUnavailable("host_projection_unreadable");
  }

  return { workroom, fullAuto, hostBinding };
};
