// SPDX-License-Identifier: Apache-2.0
// Planet labels that never sit on top of one another (#12588). On a phone,
// several planets project close together; showing every label stacks them
// into an unreadable pile. The selected planet's label always shows; the
// others show in order of nearness to the camera while they have room.

/** A label's projected center and size, in CSS pixels. */
export interface Label {
  readonly id: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  /** Distance from the camera; nearer labels win a tie for space. */
  readonly depth: number;
  /** Inside the frame and in front of the camera. */
  readonly inFrame: boolean;
}

/** Space kept clear around each shown label. */
export const LABEL_GAP = 2;

const overlaps = (a: Label, b: Label, gap: number) =>
  Math.abs(a.x - b.x) * 2 < a.width + b.width + gap * 2 &&
  Math.abs(a.y - b.y) * 2 < a.height + b.height + gap * 2;

/** The IDs of the labels to show. */
export function placeLabels(
  labels: ReadonlyArray<Label>,
  selected: string | undefined,
  frame?: { readonly width: number; readonly height: number },
  gap = LABEL_GAP,
): Set<string> {
  // A label cut off by the frame's edge reads as a mistake; leave it out.
  const fits = (label: Label) =>
    !frame ||
    (label.x - label.width / 2 >= 0 &&
      label.x + label.width / 2 <= frame.width &&
      label.y - label.height / 2 >= 0 &&
      label.y + label.height / 2 <= frame.height);
  const order = labels
    .filter((label) => label.inFrame && (label.id === selected || fits(label)))
    .sort(
      (a, b) =>
        Number(b.id === selected) - Number(a.id === selected) ||
        a.depth - b.depth ||
        a.id.localeCompare(b.id),
    );
  const shown: Label[] = [];
  for (const label of order)
    if (label.id === selected || !shown.some((s) => overlaps(s, label, gap)))
      shown.push(label);
  return new Set(shown.map((label) => label.id));
}

/**
 * The zoom step for a two-finger pinch: fingers moving apart zoom in
 * (a negative step, which brings the camera closer).
 */
export function pinchStep(before: number, after: number, distance: number) {
  if (before <= 0 || after <= 0) return 0;
  return distance * (before / after - 1);
}
