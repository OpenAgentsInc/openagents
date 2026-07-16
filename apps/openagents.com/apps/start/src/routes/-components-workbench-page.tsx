import { khalaTheme } from "@effect-native/tokens";
import {
  ContextMeter,
  desktopAgentGroupFixtures,
  desktopApprovalCardInteractiveFixture,
  desktopApprovalCardStaticFixtures,
  desktopCommandCardFixtures,
  desktopContextMeterFixtures,
  desktopDispatchLongTailFixtures,
  DesktopAgentGroup,
  DesktopApprovalCard,
  DesktopCommandCard,
  DesktopComposerBar,
  DesktopComposerButton,
  DesktopComposerFrame,
  DesktopComposerInput,
  DesktopConversation,
  DesktopConversationHeader,
  DesktopFileChangeCard,
  desktopFileChangeCardFixtures,
  desktopPlanCardFixtures,
  DesktopPlanCard,
  DesktopQueuedFollowup,
  DesktopRailScrim,
  desktopReasoningDisclosureFixtures,
  DesktopReasoningDisclosure,
  DesktopSessionRail,
  DesktopSidebarExpand,
  desktopThemeCssVariables,
  DesktopTimeline,
  desktopTimelineMessageFixtures,
  DesktopTimelineMessage,
  desktopTimelineNoticeFixtures,
  DesktopTimelineNotice,
  desktopToolCallCardFixtures,
  DesktopToolCallCard,
  DesktopWorkbench,
  DesktopWorkEntry,
  DesktopWorkGroup,
  dispatchWorkbenchItem,
} from "@openagentsinc/ui/desktop-workbench";
import { useState, type ReactElement, type ReactNode } from "react";

/**
 * `/components/workbench` — issue 8870, epic 8857 T13.
 *
 * Renders every `@openagentsinc/ui/desktop-workbench` component through a
 * realistic fixture for every ThreadItem variant and lifecycle state named
 * in the epic 8857 component audit. This is the owner's review surface
 * (`docs/fable/autopilot-ui-component-audit.md` section 3): every component
 * built across Wave 2 (T4-T12) must appear here with real props, not a
 * metadata card. Fixture data itself lives beside its component in
 * `packages/ui/src/workbench/*.fixtures.ts` so this route and any future
 * Desktop test can both import it — this file only arranges and labels it.
 *
 * `-components-workbench-page.test.ts` (colocated) is the completeness gate:
 * it parses the `@openagentsinc/ui/desktop-workbench` barrel's runtime
 * exports and asserts every one of them is referenced by name in this file,
 * so a future component or fixture added to the barrel without a story here
 * fails the test instead of silently going unreviewed.
 */

const slug = (itemKey: string): string => itemKey.replace(/^fixture-/, "");

function Story({
  children,
  id,
  title,
}: Readonly<{ children: ReactNode; id: string; title: string }>): ReactElement {
  return (
    <article
      className="grid content-start gap-3 border border-khala-border bg-khala-surface p-4"
      data-storybook-story={id}
      key={id}
    >
      <h3 className="m-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint">
        {title}
      </h3>
      {children}
    </article>
  );
}

function SectionHeader({
  description,
  title,
}: Readonly<{ description: string; title: string }>): ReactElement {
  return (
    <header className="grid gap-2 border-b border-khala-border pb-4 pt-5">
      <h2 className="m-0 text-3xl font-semibold text-khala-text">{title}</h2>
      <p className="m-0 max-w-[72ch] text-khala-text-muted">{description}</p>
    </header>
  );
}

function StoryGrid({ children }: Readonly<{ children: ReactNode }>): ReactElement {
  return <div className="grid gap-5 xl:grid-cols-2">{children}</div>;
}

/** The one interactive story in the family: a real Approve/Deny decision, wired with local state. */
function ApprovalPendingInteractiveStory(): ReactElement {
  const [decision, setDecision] = useState<"approved" | "denied" | "pending">("pending");
  const id = slug(desktopApprovalCardInteractiveFixture.itemKey);
  return (
    <Story id={id} title="Pending — interactive">
      <DesktopApprovalCard
        decision={decision}
        description={
          decision === "pending"
            ? desktopApprovalCardInteractiveFixture.description
            : "Recorded decision."
        }
        itemKey={desktopApprovalCardInteractiveFixture.itemKey}
        onDecision={setDecision}
        resource={desktopApprovalCardInteractiveFixture.resource}
        title={desktopApprovalCardInteractiveFixture.title}
      />
    </Story>
  );
}

const railDestinations = [
  { id: "chat", label: "Chats", icon: "chat" as const, current: "page" as const, selected: true },
  { id: "new", label: "New session", icon: "new-session" as const },
  { id: "home", label: "Home", icon: "home" as const },
];

const railSettingsDestination = { id: "settings", label: "Settings", icon: "settings" as const };

const railSessions = [
  { id: "session-1", title: "Wire the reasoning delta", meta: "2m ago", selected: true },
  { id: "session-2", title: "Fix the file-change diff cap", meta: "1h ago" },
  { id: "session-3", title: "Audit the notice severities", meta: "yesterday" },
];

const noop = (): void => undefined;

export function WorkbenchStorybook(): ReactElement {
  // NOTE: this outer wrapper is a plain <section>, not <DesktopWorkbench> —
  // DesktopWorkbench's own CSS (.oa-react-workbench) is a fixed two-column
  // rail+conversation product-shell grid (248px rail, min-width 760px),
  // meant to wrap exactly one rail and one conversation, not an arbitrary
  // vertical stack of gallery sections. It gets its own dedicated demo in
  // the "Timeline and shell" section below instead of hijacking this page's
  // layout.
  return (
    <section
      className="grid gap-5"
      data-storybook-family="workbench"
      style={desktopThemeCssVariables(khalaTheme)}
    >
      <header className="grid gap-2 border-b border-khala-border pb-4">
        <p className="m-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint">
          @openagentsinc/ui/desktop-workbench
        </p>
        <h2 className="m-0 text-3xl font-semibold text-khala-text">
          Product workbench — every component, every variant
        </h2>
        <p className="m-0 max-w-[72ch] text-khala-text-muted">
          Every shared Desktop/web workbench component (epic 8857 T13, tracking issue 8870), mounted
          under the current Khala theme with realistic fixtures for every ThreadItem variant and
          lifecycle state — Command execution and File changes were established by their own lanes
          (T4/T5); everything below extends the family to the rest of the component set.
        </p>
      </header>

      <SectionHeader
        description="User/assistant/system tone, and an in-flight partial-text streaming example."
        title="Messages"
      />
      <StoryGrid>
        {desktopTimelineMessageFixtures.map((fixture) => (
          <Story id={slug(fixture.itemKey)} key={fixture.itemKey} title={fixture.name}>
            <DesktopTimelineMessage
              itemKey={fixture.itemKey}
              kind={fixture.kind}
              label={fixture.label}
              sequence={fixture.sequence}
              tone={fixture.tone}
            >
              <p>{fixture.text}</p>
            </DesktopTimelineMessage>
          </Story>
        ))}
      </StoryGrid>

      <SectionHeader
        description="Streams as dim ghost text while the model is thinking, then collapses to a bounded summary. A redacted reasoning item never reaches this component at all — the honest fixture for that state is rendering nothing."
        title="Reasoning"
      />
      <StoryGrid>
        <Story
          id={slug(desktopReasoningDisclosureFixtures.streaming.itemKey)}
          title="Streaming (ghost text, open)"
        >
          <DesktopReasoningDisclosure {...desktopReasoningDisclosureFixtures.streaming} />
        </Story>
        <Story
          id={slug(desktopReasoningDisclosureFixtures.completed.itemKey)}
          title="Completed (collapsed summary)"
        >
          <DesktopReasoningDisclosure {...desktopReasoningDisclosureFixtures.completed} />
        </Story>
        <Story id="reasoning-redacted-absent" title="Redacted (honest absence)">
          <p className="m-0 text-sm text-khala-text-faint">
            Intentionally empty: a redacted reasoning item is dropped before it ever becomes a
            WorkbenchReasoningDispatchItem, so DesktopReasoningDisclosure never mounts for it. There
            is nothing to render here — not a placeholder card.
          </p>
        </Story>
      </StoryGrid>

      <SectionHeader
        description="Live and historical command states rendered from the same bounded typed item contract. The 'declined' status has no distinct visual tone of its own — see the notices/long-tail section below for how it maps onto the same failed presentation a real production render would use."
        title="Command execution"
      />
      <StoryGrid>
        {desktopCommandCardFixtures.map((fixture) => (
          <Story id={slug(fixture.props.itemKey)} key={fixture.props.itemKey} title={fixture.name}>
            <DesktopCommandCard {...fixture.props} />
          </Story>
        ))}
      </StoryGrid>

      <SectionHeader
        description="Live patches and retained history share per-file tallies, bounded expandable unified diffs, and honest patch status."
        title="File changes and turn diff"
      />
      <StoryGrid>
        {desktopFileChangeCardFixtures.map((fixture) => (
          <Story id={slug(fixture.props.itemKey)} key={fixture.props.itemKey} title={fixture.name}>
            <DesktopFileChangeCard {...fixture.props} />
          </Story>
        ))}
      </StoryGrid>

      <SectionHeader
        description="All four callKinds (mcp, dynamic, web, image) across running/completed/failed, with and without args, results, and app-context badges (T7 #8864)."
        title="Tool calls"
      />
      <StoryGrid>
        {desktopToolCallCardFixtures.map((fixture, index) => (
          <Story id={`tool-call-${index}`} key={fixture.name} title={fixture.name}>
            <DesktopToolCallCard {...fixture.props} defaultOpen />
          </Story>
        ))}
      </StoryGrid>

      <SectionHeader
        description="The one plan renderer (T8 #8865): a mid-flight structured checklist, a fully completed checklist, and free-form prose alone (the collaboration-mode plan write-up, which may carry no checklist at all)."
        title="Plan"
      />
      <StoryGrid>
        {desktopPlanCardFixtures.map((fixture) => (
          <Story id={slug(fixture.itemKey)} key={fixture.itemKey} title={fixture.name}>
            <DesktopPlanCard
              entries={fixture.entries}
              itemKey={fixture.itemKey}
              prose={fixture.prose}
            />
          </Story>
        ))}
      </StoryGrid>

      <SectionHeader
        description="Approved and denied outcomes, a read-only historical pending row with no live answer channel, and one truly interactive pending card wired with Approve/Deny."
        title="Approvals"
      />
      <StoryGrid>
        {desktopApprovalCardStaticFixtures.map((fixture) => (
          <Story id={slug(fixture.itemKey)} key={fixture.itemKey} title={fixture.name}>
            <DesktopApprovalCard
              decision={fixture.decision}
              description={fixture.description}
              itemKey={fixture.itemKey}
              resource={fixture.resource}
              title={fixture.title}
              {...(fixture.decisionLabel === undefined
                ? {}
                : { decisionLabel: fixture.decisionLabel })}
            />
          </Story>
        ))}
        <ApprovalPendingInteractiveStory />
      </StoryGrid>

      <SectionHeader
        description="A single delegated agent in every lifecycle status, a multi-agent collabAgentToolCall group, and the three subAgentActivity ping kinds (started/interacted/interrupted), which are distinct from a row's own status (T10 #8867)."
        title="Agent group"
      />
      <StoryGrid>
        {desktopAgentGroupFixtures.map((fixture) => (
          <Story id={slug(fixture.itemKey)} key={fixture.itemKey} title={fixture.name}>
            <DesktopAgentGroup
              agents={fixture.agents}
              itemKey={fixture.itemKey}
              operation={fixture.operation}
              prompt={fixture.prompt}
              title={fixture.title}
            />
          </Story>
        ))}
      </StoryGrid>

      <SectionHeader
        description="The quantized block-progress context/usage meter (T11 #8868): no data, low/mid fill, near-limit, a fully rate-limited window, both together, and a historical inspector snapshot. Every number is an exact wire value — never a fabricated fill."
        title="Context meter"
      />
      <StoryGrid>
        {desktopContextMeterFixtures.map((fixture) => (
          <Story
            id={slug(fixture.props.itemKey ?? fixture.name)}
            key={fixture.name}
            title={fixture.name}
          >
            <ContextMeter {...fixture.props} />
          </Story>
        ))}
      </StoryGrid>

      <SectionHeader
        description="Notice severities (info/warning/error, plus the legacy danger=true shape) and the long-tail ledger rows with no dedicated component of their own — compaction, sleep, review mode entered/exited, hook prompts, and a declined command — rendered through the same dispatchWorkbenchItem table production uses (T12 #8869)."
        title="Notices and long-tail rows"
      />
      <StoryGrid>
        {desktopTimelineNoticeFixtures.map((fixture) => (
          <Story id={slug(fixture.itemKey)} key={fixture.itemKey} title={fixture.name}>
            <DesktopTimelineNotice
              body={fixture.body}
              danger={fixture.danger}
              itemKey={fixture.itemKey}
              label={fixture.label}
              severity={fixture.severity}
            />
          </Story>
        ))}
        {desktopDispatchLongTailFixtures.map((fixture) => (
          <Story id={slug(fixture.itemKey)} key={fixture.itemKey} title={fixture.name}>
            <div role="list">
              {dispatchWorkbenchItem(fixture.item, { itemKey: fixture.itemKey })}
            </div>
          </Story>
        ))}
      </StoryGrid>

      <SectionHeader
        description="The collapsed 'worked' fold that groups completed activity out of the way, built from the generic legacy DesktopWorkEntry shell (kept for backward compatibility; every production ThreadItem kind now renders through its own dedicated component instead)."
        title="Work group fold"
      />
      <StoryGrid>
        <Story id="work-group-collapsed" title="Collapsed (3 previous activities)">
          <DesktopWorkGroup count={3}>
            <DesktopWorkEntry
              body={<p>Ran pnpm run typecheck.</p>}
              itemKey="work-entry-1"
              label="Command"
              preview="pnpm run typecheck"
              status="completed"
            />
            <DesktopWorkEntry
              body={<p>Updated 2 files.</p>}
              itemKey="work-entry-2"
              label="File change"
              preview="2 files changed"
              status="completed"
            />
            <DesktopWorkEntry
              body={<p>Searched GitHub issues.</p>}
              itemKey="work-entry-3"
              label="Tool call"
              preview="search_issues"
              status="completed"
            />
          </DesktopWorkGroup>
        </Story>
        <Story id="work-group-running" title="Running (+N previous, live turn)">
          <DesktopWorkGroup count={5} running>
            <DesktopWorkEntry
              body={<p>Still executing.</p>}
              itemKey="work-entry-running"
              label="Command"
              preview="pnpm test"
              status="running"
            />
          </DesktopWorkGroup>
        </Story>
      </StoryGrid>

      <SectionHeader
        description="The composer shell: input region, action bar, and the four button kinds (submit/stop/action/toggle), plus a queued follow-up card."
        title="Composer frame"
      />
      <StoryGrid>
        <Story id="composer-basic" title="Basic composer">
          <DesktopComposerFrame onSubmit={(event) => event.preventDefault()}>
            <DesktopComposerInput>
              <textarea aria-label="Message" placeholder="Ask anything…" rows={2} />
            </DesktopComposerInput>
            <DesktopComposerBar>
              <DesktopComposerButton kind="toggle">Plan mode</DesktopComposerButton>
              <DesktopComposerButton kind="action">Attach</DesktopComposerButton>
              <DesktopComposerButton kind="stop">Stop</DesktopComposerButton>
              <DesktopComposerButton kind="submit" type="submit">
                Send
              </DesktopComposerButton>
            </DesktopComposerBar>
          </DesktopComposerFrame>
        </Story>
        <Story id="queued-followup" title="Queued follow-up (fable runtime)">
          <DesktopQueuedFollowup
            itemKey="fixture-queued-1"
            position={1}
            text="Also add a screenshot of the /components/workbench family."
          />
        </Story>
      </StoryGrid>

      <SectionHeader
        description="The session sidebar: populated with recent sessions, and its honest empty state when none exist yet."
        title="Session rail"
      />
      <StoryGrid>
        <Story id="rail-populated" title="Populated">
          <DesktopSessionRail
            canGoBack
            destinations={railDestinations}
            onBack={noop}
            onCollapse={noop}
            onDestinationSelect={noop}
            onForward={noop}
            onSearchOpenChange={noop}
            onSearchQueryChange={noop}
            onSessionSelect={noop}
            open
            sessions={railSessions}
            settingsDestination={railSettingsDestination}
            searchOpen={false}
            searchQuery=""
          />
        </Story>
        <Story id="rail-empty" title="Empty state (no sessions found)">
          <DesktopSessionRail
            destinations={railDestinations}
            onBack={noop}
            onCollapse={noop}
            onDestinationSelect={noop}
            onForward={noop}
            onSearchOpenChange={noop}
            onSearchQueryChange={noop}
            onSessionSelect={noop}
            open
            sessions={[]}
            settingsDestination={railSettingsDestination}
            searchOpen={false}
            searchQuery=""
          />
        </Story>
      </StoryGrid>

      <SectionHeader
        description="The persistent conversation header, mounting a live context meter, plus the whole-conversation frame it sits above."
        title="Conversation header"
      />
      <StoryGrid>
        <Story id="header-with-meter" title="With a live context meter">
          <DesktopConversationHeader
            lifecycle="Running"
            meter={{ usage: { totalTokens: 12_400, contextWindowTokens: 200_000 } }}
            secondary="gpt-5.1-codex"
            title="Wire the reasoning delta into the timeline"
          />
        </Story>
        <Story id="conversation-frame" title="Whole conversation frame">
          <DesktopConversation
            composer={<p className="m-0 text-xs text-khala-text-faint">(composer mounts here)</p>}
            header={
              <DesktopConversationHeader
                lifecycle="Completed"
                title="Fix the file-change diff cap"
              />
            }
            timeline={<p className="m-0 text-xs text-khala-text-faint">(timeline mounts here)</p>}
          />
        </Story>
      </StoryGrid>

      <SectionHeader
        description="The scrolling timeline region (auto-follows the latest item; shows the quantized 'working' indicator while a turn is live), and the outer app shell controls."
        title="Timeline and shell"
      />
      <StoryGrid>
        <Story id="timeline-working" title="Timeline, turn in progress">
          <DesktopTimeline working>
            <DesktopTimelineMessage itemKey="timeline-story-1" label="You" sequence={0} tone="user">
              <p>Run the tests.</p>
            </DesktopTimelineMessage>
            <DesktopTimelineMessage
              itemKey="timeline-story-2"
              label="Assistant"
              sequence={1}
              tone="assistant"
            >
              <p>Running now…</p>
            </DesktopTimelineMessage>
          </DesktopTimeline>
        </Story>
        <Story id="shell-controls" title="Sidebar expand / rail scrim">
          <div className="flex items-center gap-4">
            <DesktopSidebarExpand aria-label="Expand sidebar" />
            <DesktopRailScrim aria-label="Close sidebar" />
          </div>
        </Story>
      </StoryGrid>
      {/*
        DesktopWorkbench owns a fixed two-column rail+conversation shell
        grid (.oa-react-workbench: 248px rail, min-width 760px) — real
        production usage, not a gallery-page wrapper — so it gets its own
        full-width row here instead of squeezing into the 2-col StoryGrid.
      */}
      <Story id="workbench-frame" title="App shell frame (rail + conversation)">
        <DesktopWorkbench style={{ minHeight: 260 }}>
          <DesktopSessionRail
            destinations={railDestinations}
            onBack={noop}
            onCollapse={noop}
            onDestinationSelect={noop}
            onForward={noop}
            onSearchOpenChange={noop}
            onSearchQueryChange={noop}
            onSessionSelect={noop}
            open
            searchOpen={false}
            searchQuery=""
            sessions={railSessions}
            settingsDestination={railSettingsDestination}
          />
          <DesktopConversation
            composer={<p className="m-0 text-xs text-khala-text-faint">(composer mounts here)</p>}
            header={
              <DesktopConversationHeader
                lifecycle="Running"
                title="Wire the reasoning delta into the timeline"
              />
            }
            timeline={<p className="m-0 text-xs text-khala-text-faint">(timeline mounts here)</p>}
          />
        </DesktopWorkbench>
      </Story>
    </section>
  );
}
