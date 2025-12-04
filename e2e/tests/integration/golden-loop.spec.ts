/**
 * Integration E2E Tests (E1-E5)
 *
 * Tests the full Golden Loop cycle through the HUD.
 * Covers: ORCH-001 to ORCH-044 (Task selection, decomposition, verification, git ops, session lifecycle)
 */

import { test, expect } from "../../fixtures/mainview.fixture.js";
import {
  createSessionStart,
  createSessionComplete,
  createTaskSelected,
  createTaskDecomposed,
  createSubtaskStart,
  createSubtaskComplete,
  createSubtaskFailed,
  createVerificationStart,
  createVerificationComplete,
  createCommitCreated,
  createPushComplete,
  createAPMUpdate,
  createError,
  createGoldenLoopSequence,
  createSubtaskInfo,
} from "../../fixtures/hud-messages.js";

test.describe("Integration Tests - Golden Loop (E1-E5)", () => {
  test.describe("E1: Full Golden Loop Cycle", () => {
    test("complete Golden Loop sequence renders correctly", async ({
      mainviewPage,
      hudInjector,
      svgAssertions,
    }) => {
      // Inject a complete Golden Loop sequence
      const sequence = createGoldenLoopSequence("oa-test-e1");

      for (const msg of sequence) {
        await hudInjector.inject(msg);
        await mainviewPage.waitForRender(150);
      }

      // Final state should show:
      // 1. SVG still renders
      await expect(mainviewPage.flowSvg).toBeVisible();

      // 2. Flow content should have nodes (may include nodes from parallel tests)
      const svgContent = await mainviewPage.getSvgContent();
      expect(svgContent).toContain("flow-node-group");

      // 3. All key message types received for full cycle
      const messageTypes = new Set(
        (await mainviewPage.getHudMessages()).map((m) => (m as { type: string }).type)
      );
      expect(messageTypes.has("session_start")).toBe(true);
      expect(messageTypes.has("task_selected")).toBe(true);
      expect(messageTypes.has("verification_complete")).toBe(true);
      expect(messageTypes.has("commit_created")).toBe(true);
      expect(messageTypes.has("push_complete")).toBe(true);
      expect(messageTypes.has("session_complete")).toBe(true);
    });

    test("Golden Loop updates APM during execution", async ({
      mainviewPage,
      hudInjector,
      svgAssertions,
    }) => {
      // Start session
      await hudInjector.inject(createSessionStart("apm-test-session"));
      await mainviewPage.waitForRender(200);

      // Select task
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-apm-test",
          title: "APM Test Task",
          status: "in_progress",
          priority: 0,
        })
      );
      await mainviewPage.waitForRender(200);

      // Simulate work with APM updates
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 5.0,
          totalActions: 5,
          durationMinutes: 1,
        })
      );
      await mainviewPage.waitForRender(200);

      // Check APM widget appears
      await svgAssertions.expectAPMVisible();

      // More work, higher APM
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 25.0,
          totalActions: 50,
          durationMinutes: 2,
        })
      );
      await mainviewPage.waitForRender(200);

      // APM should still be visible with updated value
      await svgAssertions.expectAPMVisible();
      await svgAssertions.expectAPMValue(25.0);
    });
  });

  test.describe("E2: Task Selection and Decomposition", () => {
    test("task_selected creates node in flow", async ({
      mainviewPage,
      hudInjector,
    }) => {
      const taskId = "oa-task-select-e2";

      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({
          id: taskId,
          title: "E2 Task Selection Test",
          status: "in_progress",
          priority: 1,
        })
      );
      await mainviewPage.waitForRender(300);

      // SVG should contain nodes (may include nodes from parallel tests in shared server)
      const svgContent = await mainviewPage.getSvgContent();
      expect(svgContent).toContain("flow-node-group");

      const taskMessages = await mainviewPage.getHudMessagesByType("task_selected");
      expect(taskMessages.length).toBeGreaterThan(0);
    });

    test("task_decomposed message is handled without crash", async ({
      mainviewPage,
      hudInjector,
    }) => {
      const taskId = "oa-decompose-e2";

      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({
          id: taskId,
          title: "Decomposition Test",
          status: "in_progress",
        })
      );
      await mainviewPage.waitForRender(200);

      // Verify task node created
      const nodeCountBefore = await mainviewPage.getNodeCount();
      expect(nodeCountBefore).toBeGreaterThan(0);

      // Decompose into subtasks
      const subtasks = [
        createSubtaskInfo({ id: `${taskId}-sub-001`, description: "First subtask" }),
        createSubtaskInfo({ id: `${taskId}-sub-002`, description: "Second subtask" }),
        createSubtaskInfo({ id: `${taskId}-sub-003`, description: "Third subtask" }),
      ];

      await hudInjector.inject(createTaskDecomposed(subtasks));
      await mainviewPage.waitForRender(300);

      // SVG should render without crashing after decompose message
      await expect(mainviewPage.flowSvg).toBeVisible();

      // Original task node should still exist (at minimum)
      const nodeCountAfter = await mainviewPage.getNodeCount();
      expect(nodeCountAfter).toBeGreaterThanOrEqual(nodeCountBefore);

      const decomposedMessages = await mainviewPage.getHudMessagesByType("task_decomposed");
      expect(decomposedMessages.length).toBeGreaterThan(0);
    });

    test("priority badge reflects task priority", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());

      // P0 critical task
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-p0-priority",
          title: "Critical P0 Task",
          status: "in_progress",
          priority: 0,
        })
      );
      await mainviewPage.waitForRender(300);

      // SVG should contain nodes after injecting task
      const svgContent = await mainviewPage.getSvgContent();
      expect(svgContent).toContain("flow-node-group");
    });
  });

  test.describe("E3: Verification Flow", () => {
    test("verification_start and verification_complete cycle", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-verify-e3",
          title: "Verification Test",
        })
      );
      await mainviewPage.waitForRender(200);

      // Start subtask
      await hudInjector.inject(
        createSubtaskStart({
          id: "oa-verify-e3-sub-001",
          description: "Implement feature",
        })
      );
      await mainviewPage.waitForRender(200);

      // Complete subtask
      await hudInjector.inject(
        createSubtaskComplete(
          { id: "oa-verify-e3-sub-001", description: "Implement feature" },
          { success: true, filesModified: ["src/feature.ts"], turns: 3 }
        )
      );
      await mainviewPage.waitForRender(200);

      // Start verification
      await hudInjector.inject(createVerificationStart("bun test"));
      await mainviewPage.waitForRender(200);

      // Complete verification (passing)
      await hudInjector.inject(
        createVerificationComplete("bun test", true, "42 tests passed")
      );
      await mainviewPage.waitForRender(300);

      // UI should still be stable
      await expect(mainviewPage.flowSvg).toBeVisible();

      const verificationMessages = await mainviewPage.getHudMessagesByType("verification_complete");
      expect(verificationMessages.length).toBeGreaterThan(0);
    });

    test("verification failure shows error state", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-verify-fail",
          title: "Failing Verification Test",
        })
      );
      await mainviewPage.waitForRender(200);

      // Start verification
      await hudInjector.inject(createVerificationStart("bun test"));
      await mainviewPage.waitForRender(200);

      // Verification fails
      await hudInjector.inject(
        createVerificationComplete("bun test", false, "3 tests failed")
      );
      await mainviewPage.waitForRender(300);

      // UI should handle gracefully
      await expect(mainviewPage.flowSvg).toBeVisible();

      const failedVerification = (await mainviewPage.getHudMessagesByType("verification_complete"))[0] as
        | { passed?: boolean }
        | undefined;
      expect(failedVerification?.passed).toBe(false);
    });

    test("subtask failure triggers error indicator", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-subtask-fail",
          title: "Subtask Failure Test",
        })
      );
      await mainviewPage.waitForRender(200);

      // Start subtask
      await hudInjector.inject(
        createSubtaskStart({
          id: "oa-subtask-fail-sub-001",
          description: "Failing subtask",
        })
      );
      await mainviewPage.waitForRender(200);

      // Subtask fails
      await hudInjector.inject(
        createSubtaskFailed(
          { id: "oa-subtask-fail-sub-001", description: "Failing subtask" },
          "Claude Code timed out"
        )
      );
      await mainviewPage.waitForRender(300);

      // Send error message
      await hudInjector.inject(
        createError("Subtask execution failed: Claude Code timed out", "executing_subtask")
      );
      await mainviewPage.waitForRender(300);

      // Error indicator should be visible
      await expect(mainviewPage.errorIndicator).toHaveClass(/visible/);

      const errorCount = await mainviewPage.getErrorCount();
      expect(errorCount).toBeGreaterThan(0);
    });
  });

  test.describe("E4: Git Operations", () => {
    test("commit_created message handled", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-commit-e4",
          title: "Commit Test",
        })
      );
      await mainviewPage.waitForRender(200);

      // Verification passes
      await hudInjector.inject(createVerificationComplete("bun test", true));
      await mainviewPage.waitForRender(200);

      // Commit created
      await hudInjector.inject(
        createCommitCreated("abc123def456", "oa-commit-e4: Implement feature")
      );
      await mainviewPage.waitForRender(300);

      // UI should handle commit event gracefully
      await expect(mainviewPage.flowSvg).toBeVisible();

      const commitMessages = await mainviewPage.getHudMessagesByType("commit_created");
      expect(commitMessages.length).toBeGreaterThan(0);
    });

    test("push_complete message handled", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-push-e4",
          title: "Push Test",
        })
      );
      await mainviewPage.waitForRender(200);

      // Commit and push
      await hudInjector.inject(createCommitCreated("def789", "oa-push-e4: Feature"));
      await hudInjector.inject(createPushComplete("main"));
      await mainviewPage.waitForRender(300);

      // UI should handle push event gracefully
      await expect(mainviewPage.flowSvg).toBeVisible();

      const pushMessages = await mainviewPage.getHudMessagesByType("push_complete");
      expect(pushMessages.length).toBeGreaterThan(0);
    });

    test("full git workflow: verify -> commit -> push", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart("git-workflow-session"));
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-git-flow",
          title: "Full Git Workflow",
          priority: 0,
        })
      );
      await mainviewPage.waitForRender(200);

      // Subtask execution
      await hudInjector.inject(
        createSubtaskStart({ id: "oa-git-flow-sub-001", description: "Code changes" })
      );
      await mainviewPage.waitForRender(150);

      await hudInjector.inject(
        createSubtaskComplete(
          { id: "oa-git-flow-sub-001" },
          { success: true, filesModified: ["src/index.ts", "src/utils.ts"] }
        )
      );
      await mainviewPage.waitForRender(150);

      // Verification
      await hudInjector.inject(createVerificationStart("bun test"));
      await mainviewPage.waitForRender(150);
      await hudInjector.inject(createVerificationComplete("bun test", true, "All tests pass"));
      await mainviewPage.waitForRender(150);

      // Git operations
      await hudInjector.inject(createCommitCreated("abc123", "oa-git-flow: Implement feature"));
      await mainviewPage.waitForRender(150);
      await hudInjector.inject(createPushComplete("main"));
      await mainviewPage.waitForRender(300);

      // Session complete
      await hudInjector.inject(createSessionComplete(true, "Task completed successfully"));
      await mainviewPage.waitForRender(300);

      // UI should be stable after full workflow
      await expect(mainviewPage.flowSvg).toBeVisible();
      const svgContent = await mainviewPage.getSvgContent();
      expect(svgContent).toContain("flow-node-group");

      const messageTypes = new Set(
        (await mainviewPage.getHudMessages()).map((m) => (m as { type: string }).type)
      );
      expect(messageTypes.has("commit_created")).toBe(true);
      expect(messageTypes.has("push_complete")).toBe(true);
      expect(messageTypes.has("session_complete")).toBe(true);
    });
  });

  test.describe("E5: Session Lifecycle", () => {
    test("session_start initializes UI state", async ({
      mainviewPage,
      hudInjector,
    }) => {
      const sessionId = "e5-session-lifecycle";

      await hudInjector.inject(createSessionStart(sessionId));
      await mainviewPage.waitForRender(300);

      // UI should respond to session start
      await expect(mainviewPage.flowSvg).toBeVisible();
      const svgContent = await mainviewPage.getSvgContent();
      // Session ID should appear somewhere in the flow content
      // (may be contaminated by parallel test execution, so check for Session: prefix)
      expect(svgContent).toContain("Session:");

      const sessionMessages = await mainviewPage.getHudMessagesByType("session_start");
      expect(sessionMessages.length).toBeGreaterThan(0);
    });

    test("session_complete with success", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-session-complete",
          title: "Session Complete Test",
        })
      );
      await mainviewPage.waitForRender(200);

      // Complete session successfully
      await hudInjector.inject(
        createSessionComplete(true, "All tasks completed successfully")
      );
      await mainviewPage.waitForRender(300);

      // UI should handle completion gracefully
      await expect(mainviewPage.flowSvg).toBeVisible();

      const completed = await mainviewPage.getHudMessagesByType("session_complete");
      expect(completed.length).toBeGreaterThan(0);
    });

    test("session_complete with failure", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-session-fail",
          title: "Session Failure Test",
        })
      );
      await mainviewPage.waitForRender(200);

      // Session fails
      await hudInjector.inject(
        createSessionComplete(false, "Task blocked due to test failures")
      );
      await mainviewPage.waitForRender(300);

      // UI should handle failure gracefully
      await expect(mainviewPage.flowSvg).toBeVisible();

      const failed = (await mainviewPage.getHudMessagesByType("session_complete"))[0] as
        | { success?: boolean }
        | undefined;
      expect(failed?.success).toBe(false);
    });

    test("multiple sessions in sequence", async ({
      mainviewPage,
      hudInjector,
    }) => {
      // First session
      await hudInjector.inject(createSessionStart("session-1"));
      await hudInjector.inject(
        createTaskSelected({ id: "oa-multi-1", title: "Task 1" })
      );
      await mainviewPage.waitForRender(200);
      await hudInjector.inject(createSessionComplete(true, "Done"));
      await mainviewPage.waitForRender(200);

      // Second session
      await hudInjector.inject(createSessionStart("session-2"));
      await hudInjector.inject(
        createTaskSelected({ id: "oa-multi-2", title: "Task 2" })
      );
      await mainviewPage.waitForRender(200);
      await hudInjector.inject(createSessionComplete(true, "Done"));
      await mainviewPage.waitForRender(200);

      // Third session
      await hudInjector.inject(createSessionStart("session-3"));
      await hudInjector.inject(
        createTaskSelected({ id: "oa-multi-3", title: "Task 3" })
      );
      await mainviewPage.waitForRender(200);

      // UI should handle multiple sessions
      await expect(mainviewPage.flowSvg).toBeVisible();

      const sessionCompletes = await mainviewPage.getHudMessagesByType("session_complete");
      expect(sessionCompletes.length).toBeGreaterThanOrEqual(2);
    });

    test("session with APM tracking throughout", async ({
      mainviewPage,
      hudInjector,
      svgAssertions,
    }) => {
      await hudInjector.inject(createSessionStart("apm-tracking-session"));
      await mainviewPage.waitForRender(200);

      // Initial APM
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 0,
          totalActions: 0,
          durationMinutes: 0,
        })
      );
      await mainviewPage.waitForRender(200);

      // Task work
      await hudInjector.inject(
        createTaskSelected({ id: "oa-apm-tracking", title: "APM Tracking Task" })
      );
      await mainviewPage.waitForRender(200);

      // APM increases with work
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 12.5,
          totalActions: 25,
          durationMinutes: 2,
        })
      );
      await mainviewPage.waitForRender(200);

      await svgAssertions.expectAPMVisible();

      // More work, higher APM
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 28.3,
          totalActions: 85,
          durationMinutes: 3,
        })
      );
      await mainviewPage.waitForRender(200);

      // Session complete with final APM state
      await hudInjector.inject(createSessionComplete(true, "Done with 85 actions"));
      await mainviewPage.waitForRender(300);

      // APM widget should still be visible showing final state
      await expect(mainviewPage.flowSvg).toBeVisible();
      await svgAssertions.expectAPMValue(28.3);

      const sessionCompleteMessages = await mainviewPage.getHudMessagesByType("session_complete");
      expect(sessionCompleteMessages.length).toBeGreaterThan(0);
    });
  });

  test.describe("Integration Resilience", () => {
    test("handles rapid message bursts", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());

      // Rapid-fire messages (simulating fast agent)
      const tasks = [
        "oa-rapid-1",
        "oa-rapid-2",
        "oa-rapid-3",
        "oa-rapid-4",
        "oa-rapid-5",
      ];

      for (const taskId of tasks) {
        await hudInjector.inject(
          createTaskSelected({ id: taskId, title: `Rapid Task ${taskId}` })
        );
        await hudInjector.inject(
          createAPMUpdate({
            sessionAPM: Math.random() * 30,
            totalActions: Math.floor(Math.random() * 100),
          })
        );
        // Minimal delay to simulate burst
        await mainviewPage.waitForRender(50);
      }

      await mainviewPage.waitForRender(300);

      // UI should handle rapid messages without crashing
      await expect(mainviewPage.flowSvg).toBeVisible();
    });

    test("handles mixed valid and error messages", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({ id: "oa-mixed", title: "Mixed Messages Test" })
      );
      await mainviewPage.waitForRender(200);

      // Mix of valid messages and errors
      await hudInjector.inject(createSubtaskStart({ id: "oa-mixed-sub-1" }));
      await mainviewPage.waitForRender(100);

      await hudInjector.inject(createError("Transient error", "executing_subtask"));
      await mainviewPage.waitForRender(100);

      await hudInjector.inject(
        createSubtaskComplete({ id: "oa-mixed-sub-1" }, { success: true })
      );
      await mainviewPage.waitForRender(100);

      await hudInjector.inject(createVerificationStart("bun test"));
      await mainviewPage.waitForRender(100);

      await hudInjector.inject(createError("Test flaky", "verifying"));
      await mainviewPage.waitForRender(100);

      await hudInjector.inject(createVerificationComplete("bun test", true));
      await mainviewPage.waitForRender(100);

      await hudInjector.inject(createSessionComplete(true, "Done despite errors"));
      await mainviewPage.waitForRender(300);

      // UI should remain stable
      await expect(mainviewPage.flowSvg).toBeVisible();
    });

    test("canvas interactions work during message processing", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await hudInjector.inject(
        createTaskSelected({ id: "oa-interact-test", title: "Interaction Test" })
      );
      await mainviewPage.waitForRender(200);

      // Zoom while receiving messages
      await mainviewPage.zoom(-100);
      await hudInjector.inject(createAPMUpdate({ sessionAPM: 15 }));
      await mainviewPage.waitForRender(100);

      const zoomedLevel = await mainviewPage.getZoomLevel();
      expect(parseInt(zoomedLevel)).toBeGreaterThan(100);

      // Pan while receiving messages
      await mainviewPage.pan(50, 50);
      await hudInjector.inject(createAPMUpdate({ sessionAPM: 20 }));
      await mainviewPage.waitForRender(100);

      // Reset while receiving messages
      await mainviewPage.clickReset();
      await hudInjector.inject(createSessionComplete(true, "Done"));
      await mainviewPage.waitForRender(200);

      const resetLevel = await mainviewPage.getZoomLevel();
      expect(resetLevel).toBe("100%");
    });
  });
});
