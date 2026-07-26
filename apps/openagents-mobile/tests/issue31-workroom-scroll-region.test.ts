/**
 * The reachability half of omega#49.
 *
 * omega#46 exit 4 and omega#48 both landed rendered states the owner could
 * not see: on a 402x874pt phone the Workroom's section column ended at
 * "Authority receipts" and Local memory, the withheld-source coverage line,
 * Reminders, and the community states below them were drawn into a container
 * that clips its overflow with no way to swipe to it. Rendering a node is not
 * the same claim as the owner being able to reach it.
 *
 * So this test does not stop at "the view contains the text". It lowers the
 * Workroom through the real React Native renderer and holds two things: the
 * Workroom body is a host scroll region, and the sections that were below the
 * fold are inside it.
 */
import { describe, expect, test } from "vite-plus/test";

import { Effect } from "@effect-native/core/effect";
import {
  renderReactNativeView,
  type ReactElementLike,
  type ReactNodeLike,
} from "@effect-native/render-rn";

import { defaultMobileAccessibilityProfile } from "../src/screens/khala-core.ts";
import { renderMobileIssue31WorkroomView } from "../src/screens/mobile-issue31-workroom-view.ts";
import { emptyIssue31CommunityReadModel } from "../src/workroom/issue31-community-read-model.ts";
import { readIssue31FullAutoProjection } from "../src/workroom/issue31-full-auto-read-model.ts";
import {
  emptyIssue31OwnerPrivateReadModel,
  type Issue31OwnerWithheldRow,
} from "../src/workroom/issue31-owner-private-read-model.ts";
import {
  ISSUE31_GRANT_INACTIVE_NOTICE,
  ISSUE31_GRANT_REVOKED_NOTICE,
  initialIssue31MobileNostrControlState,
  type Issue31MobileNostrControlState,
} from "../src/workroom/issue31-mobile-nostr-runtime.ts";
import { emptyIssue31WorkroomReadModel } from "../src/workroom/issue31-workroom-read-model.ts";

const createElement = (
  type: unknown,
  props: Record<string, unknown> | null = null,
  ...children: ReadonlyArray<ReactNodeLike>
): ReactElementLike => ({
  type,
  key: typeof props?.key === "string" ? props.key : null,
  props: {
    ...(props ?? {}),
    ...(children.length === 0
      ? {}
      : { children: children.length === 1 ? children[0] : children }),
  },
});

const reactNative = {
  View: "View",
  Text: "Text",
  Pressable: "Pressable",
  TextInput: "TextInput",
  FlatList: "FlatList",
  SectionList: "SectionList",
  ScrollView: "ScrollView",
  Image: "Image",
  Modal: "Modal",
};

const withheld: Issue31OwnerWithheldRow = {
  cause: "quarantined",
  count: 3,
  exact: true,
  reasonRef: "reason.omega.invalid_projection_source",
  observedBy: "host",
  deepLink: "openagents://omega/workroom?room=owner_private&withheld=quarantined",
};

const renderNative = (): ReactElementLike => {
  const workroom = emptyIssue31WorkroomReadModel();
  const view = renderMobileIssue31WorkroomView(
    {
      ...workroom,
      ownerPrivate: {
        ...emptyIssue31OwnerPrivateReadModel(),
        status: "gap",
        coverage: "partial",
        withheld: [withheld],
      },
    },
    "owner_private",
    initialIssue31MobileNostrControlState(),
    defaultMobileAccessibilityProfile,
    {
      draft: "",
      memoryQuery: "",
      reminderDraft: "",
      transcriptLimit: 20,
      notice: null,
    },
    readIssue31FullAutoProjection(null, {
      hostRef: "host.omega.device-alpha",
      snapshotRef: "snapshot.omega.issue31.000042",
    }),
    emptyIssue31CommunityReadModel(),
    { draft: "", subject: "", appealDraft: "", notice: null },
  );
  return renderReactNativeView(
    view,
    { React: { createElement }, ReactNative: reactNative },
    () => Effect.succeed(undefined),
  );
};

const childNodes = (node: ReactNodeLike): ReadonlyArray<ReactNodeLike> => {
  if (node === null || typeof node !== "object") return [];
  const children = (node as ReactElementLike).props?.children;
  if (children === undefined) return [];
  return Array.isArray(children) ? children : [children as ReactNodeLike];
};

/** Every string the lowered native tree would actually draw. */
const drawnStrings = (node: ReactNodeLike): ReadonlyArray<string> => {
  if (typeof node === "string") return [node];
  return childNodes(node).flatMap(drawnStrings);
};

/** Every lowered heading, with the color the host would actually paint it. */
const collectHeadings = (
  node: ReactNodeLike,
): ReadonlyArray<{ readonly text: string; readonly color: unknown }> => {
  if (node === null || typeof node !== "object") return [];
  const element = node as ReactElementLike;
  const props = element.props as
    | Readonly<{ accessibilityRole?: string; style?: Readonly<{ color?: unknown }> }>
    | undefined;
  const own = props?.accessibilityRole === "header"
    ? [{ text: drawnStrings(node).join(""), color: props.style?.color }]
    : [];
  return [...own, ...childNodes(node).flatMap(collectHeadings)];
};

/** The lowered node that draws exactly this string, or null. */
const findByDrawnString = (node: ReactNodeLike, needle: string): ReactNodeLike => {
  if (node === null || typeof node !== "object") return null;
  for (const child of childNodes(node)) {
    const found = findByDrawnString(child, needle);
    if (found !== null) return found;
  }
  return drawnStrings(node).join("") === needle ? node : null;
};

/** The nearest host scroll container, or null when nothing owns the overflow. */
const findScrollRegion = (node: ReactNodeLike): ReactNodeLike => {
  if (node === null || typeof node !== "object") return null;
  if ((node as ReactElementLike).type === "ScrollView") return node;
  for (const child of childNodes(node)) {
    const found = findScrollRegion(child);
    if (found !== null) return found;
  }
  return null;
};

describe("Issue 31 Workroom scroll region", () => {
  test("the Workroom body is a host scroll region, not a clipping container", () => {
    const rendered = renderNative();
    // A plain View lays the column out and clips whatever does not fit the
    // phone frame; ScrollView is the host element that lets the owner reach it.
    expect(rendered.type).toBe("ScrollView");
    // Full-height frame, arrangement in the content container: with the
    // arrangement left on the frame ScrollView would ignore it, and with the
    // height left off the content container it could never overflow.
    expect(rendered.props.style).toMatchObject({ width: "100%", height: "100%" });
    expect(rendered.props.contentContainerStyle).toMatchObject({ flexDirection: "column" });
  });

  test("everything below Authority receipts is inside that scroll region", () => {
    const rendered = renderNative();
    // Gather the strings from the scroll region itself, not from the tree
    // root: "the view contains this text" is the claim that was already true
    // while the owner could not reach it.
    const scrollRegion = findScrollRegion(rendered);
    expect(scrollRegion).not.toBeNull();
    const drawn = drawnStrings(scrollRegion);
    const index = (needle: string): number =>
      drawn.findIndex((value) => value.includes(needle));

    // The section that used to be the last reachable one.
    expect(index("Authority receipts")).toBeGreaterThanOrEqual(0);
    // Everything omega#49 reported as unreachable, in the order the owner
    // swipes to it.
    expect(index("Local memory")).toBeGreaterThan(index("Authority receipts"));
    expect(index("Withheld from this device: 3 · quarantined")).toBeGreaterThan(
      index("Local memory"),
    );
    expect(index("Reminders")).toBeGreaterThan(index("Local memory"));
  });

  test("every section heading names a color token instead of the host default", () => {
    // A Text with no color token lowers to a React Native Text with no color,
    // and RN's default is black — which on this near-black surface drew the
    // section headings as unreadable dark-on-dark. Nothing hid the defect
    // except the fold: the same headings render the same way in the
    // unscrolled build. Reachable and unreadable is not fixed.
    const headings = collectHeadings(renderNative());
    expect(headings.map((heading) => heading.text)).toContain("Local memory");
    const colorless = headings.filter((heading) => typeof heading.color !== "string");
    expect(colorless.map((heading) => heading.text)).toEqual([]);
  });
});

/**
 * Handed over from the revocation lane: a revoked grant leaves `phase` healthy,
 * so keying the notice's emphasis on `failed` alone drew the most consequential
 * sentence this room can say in the same muted grey as routine status chatter.
 */
describe("Issue 31 Workroom control notice emphasis", () => {
  const noticeColor = (
    phase: Issue31MobileNostrControlState["phase"],
    notice: string,
  ): unknown => {
    const workroom = emptyIssue31WorkroomReadModel();
    const view = renderMobileIssue31WorkroomView(
      workroom,
      "owner_private",
      { ...initialIssue31MobileNostrControlState(), phase, notice },
      defaultMobileAccessibilityProfile,
      { draft: "", memoryQuery: "", reminderDraft: "", transcriptLimit: 20, notice: null },
      readIssue31FullAutoProjection(null, {
        hostRef: "host.omega.device-alpha",
        snapshotRef: "snapshot.omega.issue31.000042",
      }),
      emptyIssue31CommunityReadModel(),
      { draft: "", subject: "", appealDraft: "", notice: null },
    );
    const found = JSON.stringify(view).includes(notice);
    expect(found).toBe(true);
    const rendered = renderReactNativeView(
      view,
      { React: { createElement }, ReactNative: reactNative },
      () => Effect.succeed(undefined),
    );
    const node = findByDrawnString(rendered, notice);
    expect(node).not.toBeNull();
    return ((node as ReactElementLike).props as Readonly<{ style?: Readonly<{ color?: unknown }> }>)
      .style?.color;
  };

  test("a revoked or inactive grant is emphasised, routine chatter is not", () => {
    const revoked = noticeColor("ready", ISSUE31_GRANT_REVOKED_NOTICE);
    const inactive = noticeColor("ready", ISSUE31_GRANT_INACTIVE_NOTICE);
    const routine = noticeColor("ready", "Waiting for the host to confirm this device.");
    const failed = noticeColor("failed", "Pairing failed.");

    expect(revoked).toBe(failed);
    expect(inactive).toBe(failed);
    expect(routine).not.toBe(failed);
  });
});
