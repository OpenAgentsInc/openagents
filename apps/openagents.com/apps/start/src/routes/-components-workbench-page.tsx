import { khalaTheme } from "@effect-native/tokens"
import {
  DesktopCommandCard,
  desktopThemeCssVariables,
} from "@openagentsinc/ui/desktop-workbench"

const stories = [
  { id: "command-running", title: "Streaming", props: {
    command: "pnpm test --filter desktop", commandSource: "agent" as const,
    cwd: "/workspace/openagents", output: "RUN  desktop\n✓ contracts\n▌", status: "running" as const,
  } },
  { id: "command-completed", title: "Completed", props: {
    command: "pnpm run typecheck", commandSource: "userShell" as const,
    cwd: "/workspace/openagents/apps/openagents-desktop", durationMs: 950, exitCode: 0,
    output: "TypeScript check passed.", status: "completed" as const,
  } },
  { id: "command-failed", title: "Failed", props: {
    command: "pnpm run build", commandSource: "agent" as const, cwd: "/workspace/openagents",
    durationMs: 1480, exitCode: 1, output: "error TS2322: Type 'string' is not assignable.", status: "failed" as const,
  } },
  { id: "command-capped", title: "Bounded output tail", props: {
    command: "pnpm test", commandSource: "unifiedExecStartup" as const, cwd: "/workspace/openagents",
    durationMs: 8240, exitCode: 0, output: "…\nTest Files  84 passed\nTests  1552 passed",
    outputCapReached: true, status: "completed" as const,
  } },
] as const

export function WorkbenchStorybook() {
  return <section className="grid gap-5" data-storybook-family="workbench" style={desktopThemeCssVariables(khalaTheme)}>
    <header className="grid gap-2 border-b border-khala-border pb-4">
      <p className="m-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint">@openagentsinc/ui/desktop-workbench</p>
      <h2 className="m-0 text-3xl font-semibold text-khala-text">Command execution</h2>
      <p className="m-0 max-w-[72ch] text-khala-text-muted">Live and historical command states rendered from the same bounded typed item contract.</p>
    </header>
    <div className="grid gap-5 xl:grid-cols-2">
      {stories.map(story => <article className="grid content-start gap-3 border border-khala-border bg-khala-surface p-4" data-storybook-story={story.id} key={story.id}>
        <h3 className="m-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint">{story.title}</h3>
        <DesktopCommandCard {...story.props} defaultOpen itemKey={story.id} />
      </article>)}
    </div>
  </section>
}
