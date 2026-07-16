// /components/workbench completeness gate — issue 8870, epic 8857 T13.
//
// The acceptance gate named in the issue: every runtime export of the
// `@openagentsinc/ui/desktop-workbench` barrel (every shared component, every
// fixture set, and the dispatch function) must be referenced by name in the
// workbench-family gallery page, so a future Wave-2-style addition that lands
// a new component or fixture without wiring it into the gallery fails this
// test instead of silently going unreviewed.

import * as DesktopWorkbench from "@openagentsinc/ui/desktop-workbench";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { ComponentsPage } from "./-components-page";

const escapeForRegExp = (value: string): string => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * A real word-boundary match, not a plain substring check. `\b` in a
 * camelCase identifier does NOT fire between two letters (e.g. between the
 * "p" and "C" of `desktopContextMeterFixtures`), so this correctly rejects
 * `ContextMeter` as "covered" merely because `desktopContextMeterFixtures`
 * was imported — the export must appear as its own identifier (a JSX tag,
 * a function call, a destructured/spread usage), not buried inside another
 * export's name.
 */
const referencesIdentifier = (source: string, name: string): boolean =>
  new RegExp(`\\b${escapeForRegExp(name)}\\b`).test(source);

describe("workbench family completeness gate (issue 8870, epic 8857 T13)", () => {
  test("the /components/workbench family renders as real components, not a metadata card", () => {
    const html = renderToStaticMarkup(<ComponentsPage selectedFamily="workbench" />);

    expect(html).toContain('data-storybook-family="workbench"');
    // A representative story id from every section named in the issue —
    // a coarse smoke check; the exhaustive gate is the export-coverage test
    // below.
    expect(html).toContain('data-storybook-story="message-user"');
    expect(html).toContain('data-storybook-story="reasoning-streaming"');
    expect(html).toContain('data-storybook-story="reasoning-redacted-absent"');
    expect(html).toContain('data-storybook-story="command-running"');
    expect(html).toContain('data-storybook-story="file-turn-running"');
    expect(html).toContain('data-storybook-story="tool-call-0"');
    expect(html).toContain('data-storybook-story="plan-streaming"');
    expect(html).toContain('data-storybook-story="approval-approved"');
    expect(html).toContain('data-storybook-story="approval-pending-interactive"');
    expect(html).toContain('data-storybook-story="agent-single-running"');
    expect(html).toContain('data-storybook-story="agent-activity-kinds"');
    expect(html).toContain('data-storybook-story="meter-empty"');
    expect(html).toContain('data-storybook-story="notice-info"');
    expect(html).toContain('data-storybook-story="dispatch-compaction"');
    expect(html).toContain('data-storybook-story="dispatch-command-declined"');
    expect(html).toContain('data-storybook-story="work-group-collapsed"');
    expect(html).toContain('data-storybook-story="composer-basic"');
    expect(html).toContain('data-storybook-story="queued-followup"');
    expect(html).toContain('data-storybook-story="rail-populated"');
    expect(html).toContain('data-storybook-story="rail-empty"');
    expect(html).toContain('data-storybook-story="header-with-meter"');
    expect(html).toContain('data-storybook-story="timeline-working"');
    expect(html).toContain('data-storybook-story="shell-controls"');
  });

  test("every runtime export of @openagentsinc/ui/desktop-workbench is referenced by name in the workbench family page", () => {
    const barrelExportNames = Object.keys(DesktopWorkbench);

    // Sanity floor so this assertion can never vacuously pass because the
    // barrel import silently resolved to an empty/broken module. Only
    // raise this number if the barrel grows; never lower it to make a
    // regression disappear.
    expect(barrelExportNames.length).toBeGreaterThanOrEqual(28);

    const source = readFileSync(
      join(process.cwd(), "src/routes/-components-workbench-page.tsx"),
      "utf8",
    );

    const missing = barrelExportNames.filter((name) => !referencesIdentifier(source, name));

    expect(missing).toEqual([]);
  });
});
