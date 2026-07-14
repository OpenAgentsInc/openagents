/**
 * A deliberately small renderer-adoption contract.
 *
 * The Effect Native tag remains the public, serializable authoring API. The
 * Native SDK side names an implementation primitive or an owned composition;
 * it never leaks Zig options or Native markup into an Effect Native View.
 */
export type AdoptionLane = "direct" | "composite" | "host-only" | "unsupported";

export interface NativeSdkComponentAdoption {
  readonly effectNative: string;
  readonly nativeSdk: string;
  readonly lane: AdoptionLane;
  readonly firstProof: string;
}

export const nativeSdkComponentAdoption = [
  {
    effectNative: "Stack",
    nativeSdk: "row / column / stack",
    lane: "direct",
    firstProof: "layout + stable identity",
  },
  {
    effectNative: "Text",
    nativeSdk: "text / paragraph",
    lane: "direct",
    firstProof: "typography + selection semantics",
  },
  {
    effectNative: "Button",
    nativeSdk: "button",
    lane: "direct",
    firstProof: "typed press + focus state",
  },
  {
    effectNative: "Card",
    nativeSdk: "card",
    lane: "direct",
    firstProof: "surface tokens + children",
  },
  {
    effectNative: "Badge",
    nativeSdk: "badge",
    lane: "direct",
    firstProof: "tone + compact metrics",
  },
  {
    effectNative: "Divider",
    nativeSdk: "separator",
    lane: "direct",
    firstProof: "orientation + token hairline",
  },
  {
    effectNative: "TextField",
    nativeSdk: "input / textarea",
    lane: "direct",
    firstProof: "controlled edit + selection",
  },
  {
    effectNative: "Toggle",
    nativeSdk: "switch / toggle",
    lane: "direct",
    firstProof: "controlled boolean + keyboard",
  },
  {
    effectNative: "Slider",
    nativeSdk: "slider",
    lane: "direct",
    firstProof: "controlled value + accessibility",
  },
  {
    effectNative: "List",
    nativeSdk: "list / virtual list",
    lane: "composite",
    firstProof: "windowing + scroll anchors",
  },
  {
    effectNative: "SplitPane",
    nativeSdk: "resizable / split",
    lane: "composite",
    firstProof: "controlled divider + constraints",
  },
  {
    effectNative: "Table",
    nativeSdk: "table / data grid",
    lane: "composite",
    firstProof: "row identity + keyboard model",
  },
  {
    effectNative: "Select",
    nativeSdk: "select / combobox",
    lane: "composite",
    firstProof: "popup ownership + roving focus",
  },
  {
    effectNative: "Modal",
    nativeSdk: "dialog / sheet / drawer",
    lane: "composite",
    firstProof: "focus trap + dismissal",
  },
  {
    effectNative: "ContextMenu",
    nativeSdk: "native context menu",
    lane: "composite",
    firstProof: "one authored menu + OS presenter",
  },
  {
    effectNative: "Host(chart)",
    nativeSdk: "chart",
    lane: "host-only",
    firstProof: "bounded props + renderer-owned resource",
  },
  {
    effectNative: "Host(webview)",
    nativeSdk: "webview",
    lane: "host-only",
    firstProof: "capability-gated child surface",
  },
  {
    effectNative: "CodeEditor",
    nativeSdk: "none",
    lane: "unsupported",
    firstProof: "retain current host driver",
  },
] as const satisfies ReadonlyArray<NativeSdkComponentAdoption>;

export const adoptionCounts = nativeSdkComponentAdoption.reduce<Record<AdoptionLane, number>>(
  (counts, entry) => {
    counts[entry.lane] += 1;
    return counts;
  },
  { direct: 0, composite: 0, "host-only": 0, unsupported: 0 },
);
