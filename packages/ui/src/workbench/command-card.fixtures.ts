import type { ComponentProps } from "react";

import type { DesktopCommandCard } from "./command-card.tsx";

export type DesktopCommandCardFixture = Readonly<{
  name: string;
  props: ComponentProps<typeof DesktopCommandCard>;
}>;

/** Shared command lifecycle fixtures used by the web gallery and QA-3 pixels. */
export const desktopCommandCardFixtures: ReadonlyArray<DesktopCommandCardFixture> = [
  {
    name: "streaming",
    props: {
      itemKey: "fixture-command-running",
      command: "pnpm test --filter desktop",
      commandSource: "agent",
      cwd: "/workspace/openagents",
      output: "RUN  desktop\n✓ contracts\n▌",
      status: "running",
      defaultOpen: true,
    },
  },
  {
    name: "completed",
    props: {
      itemKey: "fixture-command-completed",
      command: "pnpm run typecheck",
      commandSource: "userShell",
      cwd: "/workspace/openagents/apps/openagents-desktop",
      durationMs: 950,
      exitCode: 0,
      output: "TypeScript check passed.",
      status: "completed",
      defaultOpen: true,
    },
  },
  {
    name: "failed",
    props: {
      itemKey: "fixture-command-failed",
      command: "pnpm run build",
      commandSource: "agent",
      cwd: "/workspace/openagents",
      durationMs: 1_480,
      exitCode: 1,
      output: "error TS2322: Type 'string' is not assignable.",
      status: "failed",
      defaultOpen: true,
    },
  },
  {
    name: "bounded output tail",
    props: {
      itemKey: "fixture-command-capped",
      command: "pnpm test",
      commandSource: "unifiedExecStartup",
      cwd: "/workspace/openagents",
      durationMs: 8_240,
      exitCode: 0,
      output: "…\nTest Files  84 passed\nTests  1552 passed",
      outputCapReached: true,
      status: "completed",
      defaultOpen: true,
    },
  },
];
