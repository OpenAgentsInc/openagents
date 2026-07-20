/**
 * Evidence-gathering script for openagents#8997 (Full Auto run view pixel
 * proof) and openagents#8976 (six named Full Auto/provider-handoff sidebar
 * tests). Drives the REAL Desktop UI through `launch-isolated-app.ts` --
 * real clicks, real typing, and real native-provider dispatch (Codex reads
 * its ordinary file-backed auth; an explicitly armed Claude proof lets the
 * real SDK resolve its ordinary session, never through a host Keychain probe
 * or credential copy) -- against a disposable scratch git
 * workspace so Full Auto can genuinely write files without touching any
 * product checkout.
 *
 * What this proves, precisely:
 *   - #8997: a real screenshot of the restyled run view (canonical
 *     ConversationTimeline + styled header/badge/buttons + formatted turn
 *     rows) from an actual isolated launch, not a fixture render.
 *   - #8976 TEST 04 analog: a real codex-local Full Auto run with a turn
 *     cap, walked away from (no manual composer messages between turns),
 *     observed to a terminal or bounded state.
 *   - #8976 TEST 06 analog: real UI clicks opening >5 other chats while the
 *     run's first turn may still be active, observing the run stays
 *     addressable afterward.
 *
 * This focused driver proves one provider-specific Full Auto row plus the
 * thread-pressure composition. The six-row bidirectional/restart driver
 * builds on the same launcher below.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { launchIsolatedDesktopApp } from "./launch-isolated-app.ts";

type LogEntry = Readonly<{ at: string; event: string; detail?: unknown }>;

const main = async (): Promise<void> => {
  const launchCwd = process.argv[2];
  const outDir = process.argv[3];
  const turnCap = Number(process.argv[4] ?? "3");
  const laneRef = process.argv[5] ?? "codex-local";
  if (launchCwd === undefined || outDir === undefined) {
    throw new Error("usage: fa-evidence-run.ts <scratch-workspace-dir> <out-dir> [turn-cap] [lane-ref]");
  }
  mkdirSync(outDir, { recursive: true });
  const log: LogEntry[] = [];
  const record = (event: string, detail?: unknown): void => {
    const entry = {
      at: new Date().toISOString(),
      event,
      ...(detail === undefined ? {} : { detail }),
    };
    log.push(entry);
    console.log(
      `[fa-evidence] ${entry.at} ${event}${detail === undefined ? "" : " " + JSON.stringify(detail)}`,
    );
  };
  const shot = async (page: import("playwright").Page, name: string): Promise<string> => {
    const file = path.join(outDir, name);
    await page.screenshot({ path: file });
    record("screenshot", { file });
    return file;
  };

  record("launching_isolated_app", { launchCwd });
  const desktop = await launchIsolatedDesktopApp({
    launchCwd,
    ...(laneRef === "claude-local"
      ? { extraEnv: { OPENAGENTS_DESKTOP_USE_DEFAULT_CLAUDE_SESSION: "1" } }
      : {}),
  });
  record("launched", { userDataPath: desktop.userDataPath });
  const { page } = desktop;

  try {
    await page.waitForSelector("text=Start a conversation with Codex", { timeout: 60_000 });
    await shot(page, "00-boot-composer.png");

    // Open the dedicated Full Auto launcher via the real sidebar button
    // (FA-AC-54: "beside/under New session").
    const fullAutoNav = page.getByRole("button", { name: "Full Auto" });
    await fullAutoNav.click();
    record("clicked_full_auto_sidebar_nav");

    await page.waitForSelector('[data-en-key="full-auto-launcher-title-field"]', {
      timeout: 30_000,
    });
    await shot(page, "01-launcher-empty.png");

    const objective =
      "In this repository, create a file named PROOF.md at the repo root containing exactly one line: FULL-AUTO-PROOF-OK";
    const doneCondition =
      "PROOF.md exists at the repository root and its content is exactly the single line FULL-AUTO-PROOF-OK (no other lines).";

    await page.fill('[data-en-key="full-auto-launcher-title-field"]', "FA-QA-01 harness proof run");
    await page.fill('[data-en-key="full-auto-launcher-objective-field"]', objective);
    await page.fill('[data-en-key="full-auto-launcher-done-condition-field"]', doneCondition);

    const workspaceFieldValue = await page.inputValue(
      '[data-en-key="full-auto-launcher-workspace-field"]',
    );
    record("workspace_field_prefilled", { workspaceFieldValue, expected: launchCwd });
    if (workspaceFieldValue.trim() === "") {
      await page.fill('[data-en-key="full-auto-launcher-workspace-field"]', launchCwd);
    }

    const laneFieldValue = await page.inputValue('[data-en-key="full-auto-launcher-lane-field"]');
    record("lane_field_default", { laneFieldValue });
    await page.selectOption('[data-en-key="full-auto-launcher-lane-field"]', laneRef);
    record("lane_selected", { laneRef });

    await page.fill('[data-en-key="full-auto-launcher-turn-cap-field"]', String(turnCap));
    await shot(page, "02-launcher-filled.png");

    await page.click('[data-en-key="full-auto-launcher-start"]');
    record("clicked_start");

    // Either the run view mounts, or a typed validation/refusal error stays
    // on the launcher -- capture whichever happens, honestly.
    const runMounted = await page
      .waitForSelector("[data-full-auto-run-ref]", { timeout: 30_000 })
      .then(() => true)
      .catch(() => false);
    if (!runMounted) {
      const errorText = await page
        .locator(".oa-react-full-auto-launcher")
        .innerText()
        .catch(() => "(could not read launcher)");
      record("run_did_not_mount", { launcherText: errorText });
      await shot(page, "02b-launcher-refused.png");
      throw new Error(
        "Full Auto run did not start (see run_did_not_mount log entry / 02b screenshot)",
      );
    }

    const runRef = await page
      .locator("[data-full-auto-run-ref]")
      .getAttribute("data-full-auto-run-ref");
    record("run_created", { runRef });
    await shot(page, "03-run-view-created.png");

    // TEST 06 analog (thread pressure): open six other chats via the real
    // "New session" sidebar action while the run's first turn may still be
    // in flight -- exactly the composer/sidebar-only interaction the prior
    // FA-QA-01 session flagged as impossible without click automation.
    const newSessionNav = page.getByRole("button", { name: "New session" });
    for (let index = 0; index < 6; index++) {
      await newSessionNav.click();
      await page.waitForTimeout(300);
    }
    record("opened_thread_pressure_chats", { count: 6 });
    await shot(page, "04-after-thread-pressure.png");

    // Return to the run view (the sidebar has no composer for a Full Auto
    // run -- reopen it the same real way an owner would: via the sidebar's
    // Full Auto entry, which routes back to the active run).
    await fullAutoNav.click();
    await page.waitForSelector("[data-full-auto-run-ref]", { timeout: 15_000 });
    record("returned_to_run_view_after_pressure");

    // Poll to a bounded terminal state (or timeout), logging every state
    // transition and turn-count change, screenshotting each transition.
    const TERMINAL_STATES = new Set(["completed", "failed", "stopped", "cap_reached"]);
    const POLL_MS = 5_000;
    const DEADLINE_MS = Date.now() + 12 * 60_000;
    let lastState: string | null = null;
    let lastTurnCount = -1;
    let sawConversation = false;
    while (Date.now() < DEADLINE_MS) {
      const runEl = page.locator("[data-full-auto-run-ref]");
      const state = await runEl.getAttribute("data-full-auto-run-state").catch(() => null);
      const turnCount = await page
        .locator(".oa-react-full-auto-turn")
        .count()
        .catch(() => 0);
      const conversationEmpty = await page
        .locator(".oa-react-full-auto-conversation-empty")
        .count()
        .then((count) => count > 0)
        .catch(() => true);

      if (state !== lastState || turnCount !== lastTurnCount) {
        record("run_state_observed", { state, turnCount, conversationEmpty });
        lastState = state;
        lastTurnCount = turnCount;
      }
      if (!conversationEmpty && !sawConversation) {
        sawConversation = true;
        await shot(page, "05-run-with-real-conversation.png");
      }
      if (state !== null && TERMINAL_STATES.has(state)) {
        record("run_reached_terminal_state", { state, turnCount });
        break;
      }
      await page.waitForTimeout(POLL_MS);
    }
    if (Date.now() >= DEADLINE_MS) record("poll_deadline_reached_without_terminal_state");

    await shot(page, "06-run-final.png");
    const finalState = await page
      .locator("[data-full-auto-run-ref]")
      .getAttribute("data-full-auto-run-state")
      .catch(() => null);
    const finalTurnCount = await page
      .locator(".oa-react-full-auto-turn")
      .count()
      .catch(() => 0);
    record("final_summary", {
      runRef,
      finalState,
      finalTurnCount,
      sawRealConversation: sawConversation,
    });
  } finally {
    writeFileSync(path.join(outDir, "receipt.json"), JSON.stringify(log, null, 2));
    record("receipt_written");
    await desktop.close();
  }
};

await main().catch((error) => {
  console.error(
    "[fa-evidence] FAILED",
    error instanceof Error ? (error.stack ?? error.message) : error,
  );
  process.exit(1);
});
