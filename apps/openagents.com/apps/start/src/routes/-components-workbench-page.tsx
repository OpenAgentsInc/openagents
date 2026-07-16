import { khalaTheme } from "@effect-native/tokens"
import {
  DesktopCommandCard,
  DesktopFileChangeCard,
  desktopThemeCssVariables,
} from "@openagentsinc/ui/desktop-workbench"

const commandStories = [
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

const fileStories = [
  { id: "file-turn-running", title: "Running turn diff", scope: "turn" as const, status: "running" as const, changes: [
    { path: "src/workbench.tsx", kind: "update" as const, additions: 7, deletions: 2, diff: "@@ -18,2 +18,7 @@\n-old surface\n+const surface = khalaTheme\n+render(surface)" },
    { path: "src/file-card.test.tsx", kind: "add" as const, additions: 4, deletions: 0, diff: "--- /dev/null\n+++ b/src/file-card.test.tsx\n+test('renders diffs', () => {\n+  expect(card).toBeVisible()\n+})" },
  ] },
  { id: "file-completed", title: "Applied patch", scope: "item" as const, status: "completed" as const, changes: [
    { path: "docs/design-contract.md", kind: "update" as const, additions: 2, deletions: 1, diff: "@@ -4,1 +4,2 @@\n-Autopilot palette\n+Khala is the sole mounted theme.\n+Autopilot is donor grammar only." },
  ] },
  { id: "file-failed", title: "Failed patch", scope: "item" as const, status: "failed" as const, changes: [
    { path: "src/conflicted.ts", kind: "delete" as const, additions: 0, deletions: 2, diff: "@@ -1,2 +0,0 @@\n-export const stale = true\n-export const duplicate = true" },
  ] },
  { id: "file-capped", title: "Bounded diff", scope: "item" as const, status: "completed" as const, changes: [
    { path: "src/generated.ts", kind: "update" as const, additions: 48, deletions: 12, diff: "@@ -400,2 +400,3 @@\n-old generated tail\n+new generated tail\n+…", diffCapReached: true },
  ] },
] as const

export function WorkbenchStorybook() {
  return <section className="grid gap-5" data-storybook-family="workbench" style={desktopThemeCssVariables(khalaTheme)}>
    <header className="grid gap-2 border-b border-khala-border pb-4">
      <p className="m-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint">@openagentsinc/ui/desktop-workbench</p>
      <h2 className="m-0 text-3xl font-semibold text-khala-text">Command execution</h2>
      <p className="m-0 max-w-[72ch] text-khala-text-muted">Live and historical command states rendered from the same bounded typed item contract.</p>
    </header>
    <div className="grid gap-5 xl:grid-cols-2">
      {commandStories.map(story => <article className="grid content-start gap-3 border border-khala-border bg-khala-surface p-4" data-storybook-story={story.id} key={story.id}>
        <h3 className="m-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint">{story.title}</h3>
        <DesktopCommandCard {...story.props} defaultOpen itemKey={story.id} />
      </article>)}
    </div>
    <header className="grid gap-2 border-b border-khala-border pb-4 pt-5">
      <h2 className="m-0 text-3xl font-semibold text-khala-text">File changes and turn diff</h2>
      <p className="m-0 max-w-[72ch] text-khala-text-muted">Live patches and retained history share per-file tallies, bounded expandable unified diffs, and honest patch status.</p>
    </header>
    <div className="grid gap-5 xl:grid-cols-2">
      {fileStories.map(story => <article className="grid content-start gap-3 border border-khala-border bg-khala-surface p-4" data-storybook-story={story.id} key={story.id}>
        <h3 className="m-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint">{story.title}</h3>
        <DesktopFileChangeCard changes={story.changes} defaultOpen itemKey={story.id} scope={story.scope} status={story.status} />
      </article>)}
    </div>
  </section>
}
