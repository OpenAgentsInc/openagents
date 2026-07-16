/**
 * Fixture set for `DesktopTimelineMessage` (issue 8870, epic 8857 T13:
 * `/components` workbench-family completeness lane).
 *
 * `DesktopTimelineMessage` itself takes no "streaming" prop — it renders
 * whatever text it is given as its `children`. The honest "streaming" fixture
 * below is therefore just a message whose text is a still-growing partial
 * sentence (the same shape the live delta-accumulation path hands the
 * component turn by turn), not a fabricated loading affordance the component
 * does not have.
 */
export type DesktopTimelineMessageFixture = Readonly<{
  name: string;
  itemKey: string;
  kind: string;
  label: string;
  sequence: number;
  tone: "assistant" | "user";
  text: string;
}>;

export const desktopTimelineMessageFixtures: ReadonlyArray<DesktopTimelineMessageFixture> = [
  {
    name: "user message",
    itemKey: "fixture-message-user",
    kind: "user_message",
    label: "You",
    sequence: 0,
    tone: "user",
    text: "Can you wire the streaming reasoning delta into the timeline?",
  },
  {
    name: "assistant message (completed)",
    itemKey: "fixture-message-assistant",
    kind: "assistant_message",
    label: "Assistant",
    sequence: 1,
    tone: "assistant",
    text: "Done — the reasoning disclosure now streams ghost-text while the turn is still running and collapses to a bounded summary once it completes.",
  },
  {
    name: "assistant message (streaming, partial text)",
    itemKey: "fixture-message-assistant-streaming",
    kind: "assistant_message",
    label: "Assistant",
    sequence: 2,
    tone: "assistant",
    text: "Checking the dispatch table now — every kind renders through its own",
  },
  {
    name: "system message (rendered on the assistant tone lane)",
    itemKey: "fixture-message-system",
    kind: "system_message",
    label: "System",
    sequence: 3,
    tone: "assistant",
    text: "This session is running with network access enabled.",
  },
];
