/**
 * Desktop Screenshot E2E Tests
 *
 * Visual regression tests that capture and compare screenshots of the desktop UI
 * in various states. These tests ensure visual consistency across changes.
 *
 * Screenshot categories:
 * - Empty/idle state
 * - Session with task selected
 * - Active task execution (subtask in progress)
 * - Completed task with APM stats
 * - Error states
 * - Various zoom/pan states
 */

import { test, expect } from "../../fixtures/mainview.fixture.js";
import {
  createSessionStart,
  createTaskSelected,
  createTaskDecomposed,
  createSubtaskStart,
  createSubtaskComplete,
  createSubtaskFailed,
  createVerificationStart,
  createVerificationComplete,
  createCommitCreated,
  createAPMUpdate,
  createError,
  createSubtaskInfo,
  createGoldenLoopSequence,
} from "../../fixtures/hud-messages.js";

const DEFAULT_VIEWPORT = { width: 1280, height: 720 };

test.describe("Desktop Screenshots", () => {
  test.describe.configure({ mode: "serial" });
  test.use({ viewport: DEFAULT_VIEWPORT });

  test.describe("Idle States", () => {
    test("captures empty state on initial load", async ({ page, mainviewPage }) => {
      // Wait for UI to stabilize
      await mainviewPage.waitForRender(500);

      // Capture screenshot
      await expect(page).toHaveScreenshot("idle-empty-state.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures canvas after pan interaction", async ({ page, mainviewPage }) => {
      // Pan the canvas
      await mainviewPage.pan(150, 100);
      await mainviewPage.waitForRender(300);

      await expect(page).toHaveScreenshot("idle-after-pan.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures canvas at various zoom levels", async ({ page, mainviewPage }) => {
      // Zoom in
      await mainviewPage.zoom(-200); // Zoom in
      await mainviewPage.waitForRender(300);

      await expect(page).toHaveScreenshot("zoom-in-150-percent.png", {
        maxDiffPixelRatio: 0.05,
      });

      // Reset and zoom out
      await mainviewPage.clickReset();
      await mainviewPage.waitForRender(300);
      await mainviewPage.zoom(200); // Zoom out
      await mainviewPage.waitForRender(300);

      await expect(page).toHaveScreenshot("zoom-out-75-percent.png", {
        maxDiffPixelRatio: 0.05,
      });
    });
  });

  test.describe("Session States", () => {
    test("captures session start state", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-session-1"));
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("session-started.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures single task selected", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-session-2"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-screenshot-task",
          title: "Implement desktop screenshot tests",
          status: "in_progress",
          priority: 1,
        })
      );
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("task-selected.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures task with multiple subtasks decomposed", async ({
      page,
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart("screenshot-session-3"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-multi-subtask",
          title: "Complex multi-step task",
          status: "in_progress",
        })
      );
      await hudInjector.inject(
        createTaskDecomposed([
          createSubtaskInfo({ id: "sub-001", description: "Research existing patterns" }),
          createSubtaskInfo({ id: "sub-002", description: "Implement core logic" }),
          createSubtaskInfo({ id: "sub-003", description: "Write unit tests" }),
          createSubtaskInfo({ id: "sub-004", description: "Add documentation" }),
        ])
      );
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("task-decomposed-multiple.png", {
        maxDiffPixelRatio: 0.05,
      });
    });
  });

  test.describe("Active Execution States", () => {
    test("captures subtask in progress", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-active-1"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-active-task",
          title: "Active task execution",
        })
      );
      await hudInjector.inject(
        createSubtaskStart({
          id: "active-sub-001",
          description: "Implementing feature module",
          status: "in_progress",
        })
      );
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("subtask-in-progress.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures verification running", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-verify-1"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-verify-task",
          title: "Task with verification",
        })
      );
      await hudInjector.inject(
        createSubtaskComplete({
          id: "verify-sub-001",
          description: "Code changes complete",
        })
      );
      await hudInjector.inject(createVerificationStart("bun test"));
      await mainviewPage.waitForRender(1000);

      await expect(page).toHaveScreenshot("verification-running.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures verification passed", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-verify-2"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-verify-pass-task",
          title: "Task verification passed",
        })
      );
      await hudInjector.inject(createVerificationComplete("bun test", true, "42 tests passed"));
      await mainviewPage.waitForRender(1000);

      await expect(page).toHaveScreenshot("verification-passed.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures commit created", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-commit-1"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-commit-task",
          title: "Task with commit",
        })
      );
      await hudInjector.inject(createCommitCreated("abc123def", "feat: add screenshot tests"));
      await mainviewPage.waitForRender(1000);

      await expect(page).toHaveScreenshot("commit-created.png", {
        maxDiffPixelRatio: 0.05,
      });
    });
  });

  test.describe("APM Display States", () => {
    test("captures APM widget with low activity", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-apm-1"));
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 5.0,
          recentAPM: 4.5,
          totalActions: 10,
          durationMinutes: 2,
        })
      );
      await mainviewPage.waitForRender(1000);

      await expect(page).toHaveScreenshot("apm-low-activity.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures APM widget with high activity", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-apm-2"));
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 45.0,
          recentAPM: 52.0,
          totalActions: 250,
          durationMinutes: 5.5,
        })
      );
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("apm-high-activity.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures APM progression over time", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-apm-3"));

      // Start with low APM
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 8.0,
          totalActions: 20,
          durationMinutes: 2.5,
        })
      );
      await mainviewPage.waitForRender(300);

      await expect(page).toHaveScreenshot("apm-progression-start.png", {
        maxDiffPixelRatio: 0.05,
      });

      // Progress to higher APM
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 25.0,
          totalActions: 75,
          durationMinutes: 3,
        })
      );
      await mainviewPage.waitForRender(300);

      await expect(page).toHaveScreenshot("apm-progression-mid.png", {
        maxDiffPixelRatio: 0.05,
      });

      // Peak activity
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 60.0,
          totalActions: 200,
          durationMinutes: 3.3,
        })
      );
      await mainviewPage.waitForRender(300);

      await expect(page).toHaveScreenshot("apm-progression-peak.png", {
        maxDiffPixelRatio: 0.05,
      });
    });
  });

  test.describe("Error States", () => {
    test("captures subtask failed state", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-error-1"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-error-task",
          title: "Task with failure",
        })
      );
      await hudInjector.inject(
        createSubtaskFailed(
          { id: "error-sub-001", description: "Failing subtask" },
          "TypeError: Cannot read property 'id' of undefined"
        )
      );
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("subtask-failed.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures verification failed state", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-error-2"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-verify-fail-task",
          title: "Task verification failed",
        })
      );
      await hudInjector.inject(
        createVerificationComplete("bun test", false, "5 tests failed:\n- test1\n- test2")
      );
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("verification-failed.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures error indicator visible", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-error-3"));
      await hudInjector.inject(createError("Network timeout while pushing to remote", "committing"));
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("error-indicator-visible.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures multiple errors accumulated", async ({ page, mainviewPage, hudInjector }) => {
      await hudInjector.inject(createSessionStart("screenshot-error-4"));
      await hudInjector.inject(createError("First error: initialization failed", "selecting_task"));
      await mainviewPage.waitForRender(200);
      await hudInjector.inject(createError("Second error: timeout", "executing_subtask"));
      await mainviewPage.waitForRender(200);
      await hudInjector.inject(createError("Third error: commit failed", "committing"));
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("multiple-errors.png", {
        maxDiffPixelRatio: 0.05,
      });
    });
  });

  test.describe("Complete Workflow States", () => {
    test("captures full golden loop progression", async ({
      page,
      mainviewPage,
      hudInjector,
    }) => {
      const sequence = createGoldenLoopSequence("oa-screenshot-golden");

      // Capture initial state
      await expect(page).toHaveScreenshot("golden-loop-01-initial.png", {
        maxDiffPixelRatio: 0.05,
      });

      // Session start
      await hudInjector.inject(sequence[0]);
      await mainviewPage.waitForRender(500);
      await expect(page).toHaveScreenshot("golden-loop-02-session-start.png", {
        maxDiffPixelRatio: 0.05,
      });

      // Task selected
      await hudInjector.inject(sequence[1]);
      await mainviewPage.waitForRender(500);
      await expect(page).toHaveScreenshot("golden-loop-03-task-selected.png", {
        maxDiffPixelRatio: 0.05,
      });

      // Task decomposed
      await hudInjector.inject(sequence[2]);
      await mainviewPage.waitForRender(500);
      await expect(page).toHaveScreenshot("golden-loop-04-decomposed.png", {
        maxDiffPixelRatio: 0.05,
      });

      // First subtask in progress
      await hudInjector.inject(sequence[3]);
      await mainviewPage.waitForRender(500);
      await expect(page).toHaveScreenshot("golden-loop-05-subtask1-start.png", {
        maxDiffPixelRatio: 0.05,
      });

      // First subtask complete
      await hudInjector.inject(sequence[4]);
      await mainviewPage.waitForRender(500);
      await expect(page).toHaveScreenshot("golden-loop-06-subtask1-done.png", {
        maxDiffPixelRatio: 0.05,
      });

      // Verification passed (indices 7=verification_start, 8=verification_complete)
      await hudInjector.inject(sequence[7]);
      await hudInjector.inject(sequence[8]);
      await mainviewPage.waitForRender(500);
      await expect(page).toHaveScreenshot("golden-loop-07-verified.png", {
        maxDiffPixelRatio: 0.05,
      });

      // Commit created (index 9=commit_created)
      await hudInjector.inject(sequence[9]);
      await mainviewPage.waitForRender(500);
      await expect(page).toHaveScreenshot("golden-loop-08-committed.png", {
        maxDiffPixelRatio: 0.05,
      });

      // Session complete (index 11=session_complete)
      await hudInjector.inject(sequence[11]);
      await mainviewPage.waitForRender(500);
      await expect(page).toHaveScreenshot("golden-loop-09-complete.png", {
        maxDiffPixelRatio: 0.05,
      });
    });
  });

  test.describe("Viewport Responsive States", () => {
    test("captures narrow viewport (mobile-like)", async ({ page, mainviewPage }) => {
      await page.setViewportSize({ width: 480, height: 800 });
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("viewport-narrow.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures wide viewport (desktop)", async ({ page, mainviewPage }) => {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("viewport-wide.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures square viewport", async ({ page, mainviewPage }) => {
      await page.setViewportSize({ width: 800, height: 800 });
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("viewport-square.png", {
        maxDiffPixelRatio: 0.05,
      });
    });
  });

  test.describe("WebSocket Connection States", () => {
    test("captures disconnected state indicator", async ({ page, mainviewPage, hudInjector }) => {
      // Inject some initial state
      await hudInjector.inject(createSessionStart("screenshot-ws-1"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-ws-task",
          title: "Task during WS disconnect",
        })
      );
      await mainviewPage.waitForRender(300);

      // Disconnect WebSocket
      await hudInjector.disconnectAll();
      await mainviewPage.waitForRender(500);

      await expect(page).toHaveScreenshot("ws-disconnected.png", {
        maxDiffPixelRatio: 0.05,
      });
    });

    test("captures reconnected state after disconnect", async ({
      page,
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart("screenshot-ws-2"));
      await mainviewPage.waitForRender(300);

      // Disconnect
      await hudInjector.disconnectAll();
      await mainviewPage.waitForRender(300);

      // Wait for reconnect
      await page.waitForSelector("#ws-status.ws-connected", { timeout: 3000 });
      await mainviewPage.waitForRender(300);

      await expect(page).toHaveScreenshot("ws-reconnected.png", {
        maxDiffPixelRatio: 0.05,
      });
    });
  });
});
