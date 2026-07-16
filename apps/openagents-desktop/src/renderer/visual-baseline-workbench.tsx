import type { Theme } from "@effect-native/core";
import {
  ContextMeter,
  desktopAgentGroupFixtures,
  desktopApprovalCardInteractiveFixture,
  desktopApprovalCardStaticFixtures,
  desktopCommandCardFixtures,
  desktopContextMeterFixtures,
  desktopDispatchLongTailFixtures,
  desktopFileChangeCardFixtures,
  desktopPlanCardFixtures,
  desktopReasoningDisclosureFixtures,
  desktopThemeCssVariables,
  desktopTimelineMessageFixtures,
  desktopTimelineNoticeFixtures,
  desktopToolCallCardFixtures,
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
  DesktopPlanCard,
  DesktopQueuedFollowup,
  DesktopRailScrim,
  DesktopReasoningDisclosure,
  DesktopSessionRail,
  DesktopSidebarExpand,
  DesktopTimeline,
  DesktopTimelineMessage,
  DesktopTimelineNotice,
  DesktopToolCallCard,
  DesktopWorkbench,
  DesktopWorkEntry,
  DesktopWorkGroup,
  dispatchWorkbenchItem,
} from "@openagentsinc/ui/desktop-workbench";
import type { CSSProperties, ReactElement, ReactNode } from "react";
import { createRoot } from "react-dom/client";

import type { VisualBaselineWorkbenchStateName } from "../visual-baseline-contract.ts";

const noop = (): void => undefined;

const storyStyle: CSSProperties = {
  minWidth: 0,
  overflow: "hidden",
  border: "1px solid var(--en-color-borderSubtle)",
  background: "var(--en-color-surface)",
  padding: 10,
};

const Story = ({
  children,
  title,
}: Readonly<{ children: ReactNode; title: string }>): ReactElement => (
  <section style={storyStyle}>
    <h2
      style={{
        margin: "0 0 8px",
        color: "var(--en-color-textFaint)",
        fontFamily: "var(--oa-font-mono)",
        fontSize: 10,
        letterSpacing: ".08em",
        textTransform: "uppercase",
      }}
    >
      {title}
    </h2>
    {children}
  </section>
);

const Grid = ({
  children,
  columns = 2,
}: Readonly<{ children: ReactNode; columns?: number }>): ReactElement => (
  <div
    style={{
      display: "grid",
      gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
      gap: 10,
      alignContent: "start",
    }}
  >
    {children}
  </div>
);

const messages = (): ReactElement => (
  <Grid columns={3}>
    {desktopTimelineMessageFixtures.map((fixture) => (
      <Story key={fixture.itemKey} title={fixture.name}>
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
    <Story title="reasoning — streaming">
      <DesktopReasoningDisclosure {...desktopReasoningDisclosureFixtures.streaming} />
    </Story>
    <Story title="reasoning — completed">
      <DesktopReasoningDisclosure {...desktopReasoningDisclosureFixtures.completed} />
    </Story>
    <Story title="reasoning — redacted honest absence">
      <p style={{ color: "var(--en-color-textFaint)", margin: 0 }}>
        No component mounts for a redacted reasoning item.
      </p>
    </Story>
  </Grid>
);

const commands = (): ReactElement => (
  <Grid>
    {desktopCommandCardFixtures.map((fixture) => (
      <Story key={fixture.props.itemKey} title={fixture.name}>
        <DesktopCommandCard {...fixture.props} />
      </Story>
    ))}
  </Grid>
);
const files = (): ReactElement => (
  <Grid>
    {desktopFileChangeCardFixtures.map((fixture) => (
      <Story key={fixture.props.itemKey} title={fixture.name}>
        <DesktopFileChangeCard {...fixture.props} />
      </Story>
    ))}
  </Grid>
);
const tools = (offset: number, count: number): ReactElement => (
  <Grid columns={2}>
    {desktopToolCallCardFixtures.slice(offset, offset + count).map((fixture) => (
      <Story key={fixture.props.itemKey} title={fixture.name}>
        <DesktopToolCallCard {...fixture.props} defaultOpen />
      </Story>
    ))}
  </Grid>
);

const plansApprovals = (): ReactElement => (
  <Grid columns={3}>
    {desktopPlanCardFixtures.map((fixture) => (
      <Story key={fixture.itemKey} title={fixture.name}>
        <DesktopPlanCard
          entries={fixture.entries}
          itemKey={fixture.itemKey}
          prose={fixture.prose}
        />
      </Story>
    ))}
    {desktopApprovalCardStaticFixtures.map((fixture) => (
      <Story key={fixture.itemKey} title={fixture.name}>
        <DesktopApprovalCard {...fixture} />
      </Story>
    ))}
    <Story title="pending — interactive">
      <DesktopApprovalCard
        {...desktopApprovalCardInteractiveFixture}
        decision="pending"
        onDecision={noop}
      />
    </Story>
  </Grid>
);

const agents = (): ReactElement => (
  <Grid columns={2}>
    {desktopAgentGroupFixtures.map((fixture) => (
      <Story key={fixture.itemKey} title={fixture.name}>
        <DesktopAgentGroup
          agents={fixture.agents}
          itemKey={fixture.itemKey}
          operation={fixture.operation}
          prompt={fixture.prompt}
          title={fixture.title}
        />
      </Story>
    ))}
  </Grid>
);
const context = (): ReactElement => (
  <Grid columns={3}>
    {desktopContextMeterFixtures.map((fixture) => (
      <Story key={fixture.name} title={fixture.name}>
        <ContextMeter {...fixture.props} />
      </Story>
    ))}
  </Grid>
);

const noticesLongTail = (): ReactElement => (
  <Grid columns={3}>
    {desktopTimelineNoticeFixtures.map((fixture) => (
      <Story key={fixture.itemKey} title={fixture.name}>
        <DesktopTimelineNotice {...fixture} />
      </Story>
    ))}
    {desktopDispatchLongTailFixtures.map((fixture) => (
      <Story key={fixture.itemKey} title={fixture.name}>
        <div role="list">{dispatchWorkbenchItem(fixture.item, { itemKey: fixture.itemKey })}</div>
      </Story>
    ))}
  </Grid>
);

const railDestinations = [
  { id: "chat", label: "Chats", icon: "chat" as const, current: "page" as const, selected: true },
  { id: "new", label: "New session", icon: "new-session" as const },
  { id: "home", label: "Home", icon: "home" as const },
];
const railSettingsDestination = { id: "settings", label: "Settings", icon: "settings" as const };
const railSessions = [
  { id: "session-1", title: "Wire the reasoning delta", meta: "2m ago", selected: true },
  { id: "session-2", title: "Fix the file-change diff cap", meta: "1h ago" },
];
const rail = (
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
);

const shell = (): ReactElement => (
  <div
    style={{ display: "grid", gridTemplateColumns: "280px minmax(0, 1fr)", gap: 12, height: 710 }}
  >
    <Story title="session rail — populated">{rail}</Story>
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
        gap: 10,
        alignContent: "start",
      }}
    >
      <Story title="work group fold">
        <DesktopWorkGroup count={3}>
          <DesktopWorkEntry
            body={<p>Typecheck passed.</p>}
            itemKey="work-entry-1"
            label="Command"
            preview="pnpm run typecheck"
            status="completed"
          />
        </DesktopWorkGroup>
      </Story>
      <Story title="work group — running">
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
      <Story title="composer frame + buttons">
        <DesktopComposerFrame onSubmit={(event) => event.preventDefault()}>
          <DesktopComposerInput>
            <textarea aria-label="Message" placeholder="Ask anything…" rows={2} />
          </DesktopComposerInput>
          <DesktopComposerBar>
            <DesktopComposerButton kind="toggle">Plan</DesktopComposerButton>
            <DesktopComposerButton kind="action">Attach</DesktopComposerButton>
            <DesktopComposerButton kind="stop">Stop</DesktopComposerButton>
            <DesktopComposerButton kind="submit">Send</DesktopComposerButton>
          </DesktopComposerBar>
        </DesktopComposerFrame>
      </Story>
      <Story title="queued follow-up">
        <DesktopQueuedFollowup
          itemKey="fixture-queued-1"
          position={1}
          text="Also capture the workbench fixture catalog."
        />
      </Story>
      <Story title="conversation header + timeline">
        <DesktopConversation
          header={
            <DesktopConversationHeader
              lifecycle="Running"
              meter={{ usage: { totalTokens: 12_400, contextWindowTokens: 200_000 } }}
              secondary="gpt-5.1-codex"
              title="Wire the reasoning delta"
            />
          }
          timeline={
            <DesktopTimeline working>
              <DesktopTimelineMessage
                itemKey="shell-message"
                label="Assistant"
                sequence={1}
                tone="assistant"
              >
                <p>Running now…</p>
              </DesktopTimelineMessage>
            </DesktopTimeline>
          }
          composer={<p>Composer mounts here.</p>}
        />
      </Story>
      <Story title="sidebar controls">
        <div style={{ display: "flex", gap: 20 }}>
          <DesktopSidebarExpand
            aria-label="Expand sidebar"
            style={{ display: "inline-flex", position: "static" }}
          />
          <DesktopRailScrim
            aria-label="Close sidebar"
            style={{
              display: "block",
              position: "static",
              width: 120,
              height: 32,
              background: "var(--en-color-scrim)",
            }}
          />
        </div>
      </Story>
    </div>
  </div>
);

const frame = (): ReactElement => (
  <DesktopWorkbench style={{ height: 710 }}>
    {rail}
    <DesktopConversation
      header={
        <DesktopConversationHeader
          lifecycle="Running"
          meter={{ usage: { totalTokens: 12_400, contextWindowTokens: 200_000 } }}
          secondary="gpt-5.1-codex"
          title="Shared app shell frame"
        />
      }
      timeline={
        <DesktopTimeline working>
          <DesktopTimelineMessage itemKey="frame-user" label="You" sequence={0} tone="user">
            <p>Prove the full shared workbench frame.</p>
          </DesktopTimelineMessage>
          <DesktopTimelineMessage
            itemKey="frame-assistant"
            label="Assistant"
            sequence={1}
            tone="assistant"
          >
            <p>The rail, header, timeline, and composer are mounted together.</p>
          </DesktopTimelineMessage>
        </DesktopTimeline>
      }
      composer={
        <DesktopComposerFrame onSubmit={(event) => event.preventDefault()}>
          <DesktopComposerInput>
            <textarea aria-label="Message" placeholder="Ask anything…" rows={2} />
          </DesktopComposerInput>
          <DesktopComposerBar>
            <DesktopComposerButton kind="toggle">Plan</DesktopComposerButton>
            <DesktopComposerButton kind="action">Attach</DesktopComposerButton>
            <DesktopComposerButton kind="submit">Send</DesktopComposerButton>
          </DesktopComposerBar>
        </DesktopComposerFrame>
      }
    />
  </DesktopWorkbench>
);

export const visualBaselineWorkbenchContent = (
  state: VisualBaselineWorkbenchStateName,
): ReactElement => {
  switch (state) {
    case "workbench-messages-reasoning":
      return messages();
    case "workbench-commands":
      return commands();
    case "workbench-files":
      return files();
    case "workbench-tools-mcp-dynamic":
      return tools(0, 8);
    case "workbench-tools-web-image":
      return tools(8, 6);
    case "workbench-plans-approvals":
      return plansApprovals();
    case "workbench-agents":
      return agents();
    case "workbench-context":
      return context();
    case "workbench-notices-long-tail":
      return noticesLongTail();
    case "workbench-shell":
      return shell();
    case "workbench-frame":
      return frame();
  }
};

export const mountVisualBaselineWorkbench = (
  root: HTMLElement,
  state: VisualBaselineWorkbenchStateName,
  theme: Theme,
): void => {
  const reactRoot = createRoot(root);
  reactRoot.render(
    <main
      className="oa-react-workbench"
      data-visual-workbench={state}
      style={{
        ...desktopThemeCssVariables(theme),
        display: "block",
        width: 1280,
        height: 800,
        boxSizing: "border-box",
        overflow: "hidden",
        padding: 16,
      }}
    >
      <header style={{ height: 50 }}>
        <strong
          style={{
            fontFamily: "var(--oa-font-mono)",
            fontSize: 15,
            letterSpacing: ".08em",
            textTransform: "uppercase",
          }}
        >
          QA-3 · {state.replaceAll("-", " ")}
        </strong>
        <p style={{ color: "var(--en-color-textFaint)", margin: "5px 0 0", fontSize: 11 }}>
          Shared #8870 fixture catalog · deterministic Desktop pixel proof
        </p>
      </header>
      {visualBaselineWorkbenchContent(state)}
    </main>,
  );
  window.addEventListener("pagehide", () => reactRoot.unmount(), { once: true });
};
