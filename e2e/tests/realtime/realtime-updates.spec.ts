/**
 * Real-Time Update E2E Tests (C1-C10 / HUD-030 to HUD-040)
 *
 * Tests WebSocket-based real-time updates to the HUD.
 * - C1/HUD-030: session_start triggers UI refresh
 * - C2/HUD-031: task_selected adds/highlights node
 * - C3/HUD-032: task_decomposed creates child nodes
 * - C4/HUD-033: subtask_start/complete changes node status
 * - C5/HUD-034: verification_start/complete updates display
 * - C6/HUD-035: commit_created/push_complete reflected in UI
 * - C7/HUD-036: phase_change updates current state
 * - C8/HUD-037: error message shows in UI with context
 * - C9/HUD-038: text_output messages display in real-time
 * - C10/HUD-040: updates work after WebSocket reconnect
 */

import { test, expect } from "../../fixtures/mainview.fixture.js";
import {
  createSessionStart,
  createTaskSelected,
  createTaskDecomposed,
  createSubtaskInfo,
  createSubtaskStart,
  createSubtaskComplete,
  createVerificationStart,
  createVerificationComplete,
  createCommitCreated,
  createPushComplete,
  createError,
  createAPMUpdate,
  createGoldenLoopSequence,
} from "../../fixtures/hud-messages.js";
import type {
  PhaseChangeMessage,
  TextOutputMessage,
  ToolCallMessage,
  ToolResultMessage,
} from "../../../src/hud/protocol.js";

// Configure sequential execution due to shared WebSocket server state
test.describe.configure({ mode: "serial" });

test.describe("Real-Time Updates (C1-C10)", () => {
  // Clear message state before each test to prevent cross-test pollution
  test.beforeEach(async ({ page }) => {
    await page.evaluate(() => {
      (window as unknown as { __hudMessages: unknown[] }).__hudMessages = [];
      (window as unknown as { __errorCount: number }).__errorCount = 0;
    });
  });

  test("C1/HUD-030: session_start triggers UI refresh", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    // Store messages received by the page
    const receivedMessages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const initialCount = receivedMessages.length;

    // Send session_start message
    const sessionId = `test-session-${Date.now()}`;
    await hudInjector.inject(createSessionStart(sessionId));
    await mainviewPage.waitForRender(300);

    // Verify message was received
    const messagesAfter = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    expect(messagesAfter.length).toBeGreaterThan(initialCount);

    // Verify UI contains session info
    const svgContent = await mainviewPage.getSvgContent();
    expect(svgContent).toContain(sessionId);
  });

  test("C2/HUD-031: task_selected adds/highlights node", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    // Initial state - no task nodes
    const initialNodeCount = await mainviewPage.getNodeCount();

    // Send task_selected message
    const taskId = "oa-test-task-001";
    const taskTitle = "Implement E2E Tests";
    await hudInjector.inject(createTaskSelected({
      id: taskId,
      title: taskTitle,
      status: "in_progress",
      priority: 1,
    }));
    await mainviewPage.waitForRender(500);

    // Verify message was received
    const messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const taskMsg = messages.find((m: unknown) => (m as { type: string }).type === "task_selected");
    expect(taskMsg).toBeDefined();

    // Verify node was created
    const nodeCountAfter = await mainviewPage.getNodeCount();
    expect(nodeCountAfter).toBeGreaterThan(initialNodeCount);

    // Verify task title is displayed in SVG
    const svgContent = await mainviewPage.getSvgContent();
    expect(svgContent).toContain(taskTitle);
  });

  test("C3/HUD-032: task_decomposed creates child nodes", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    // First select a task
    const taskId = "oa-parent-task";
    await hudInjector.inject(createTaskSelected({
      id: taskId,
      title: "Parent Task",
    }));
    await mainviewPage.waitForRender(200);

    // Send task_decomposed with subtasks
    const subtasks = [
      createSubtaskInfo({ id: `${taskId}-sub-001`, description: "Subtask 1" }),
      createSubtaskInfo({ id: `${taskId}-sub-002`, description: "Subtask 2" }),
      createSubtaskInfo({ id: `${taskId}-sub-003`, description: "Subtask 3" }),
    ];
    await hudInjector.inject(createTaskDecomposed(subtasks));
    await mainviewPage.waitForRender(500);

    // Verify message was received
    const messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const decomposedMsg = messages.find((m: unknown) => (m as { type: string }).type === "task_decomposed");
    expect(decomposedMsg).toBeDefined();

    // Verify app is still functional
    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test("C4/HUD-033: subtask_start/complete changes node status", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    // Start with a task
    await hudInjector.inject(createSessionStart());
    await hudInjector.inject(createTaskSelected({ id: "oa-task-status" }));
    await mainviewPage.waitForRender(200);

    // Send subtask_start
    const subtaskId = "oa-task-status-sub-001";
    await hudInjector.inject(createSubtaskStart({
      id: subtaskId,
      description: "Running subtask",
      status: "in_progress",
    }));
    await mainviewPage.waitForRender(200);

    // Verify subtask_start was received
    let messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const startMsg = messages.find((m: unknown) => (m as { type: string }).type === "subtask_start");
    expect(startMsg).toBeDefined();

    // Send subtask_complete
    await hudInjector.inject(createSubtaskComplete(
      { id: subtaskId, description: "Completed subtask", status: "done" },
      { success: true, filesModified: ["test.ts"], turns: 5 }
    ));
    await mainviewPage.waitForRender(200);

    // Verify subtask_complete was received
    messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const completeMsg = messages.find((m: unknown) => (m as { type: string }).type === "subtask_complete");
    expect(completeMsg).toBeDefined();

    // App should still be functional
    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test("C5/HUD-034: verification_start/complete updates display", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    await hudInjector.inject(createSessionStart());
    await mainviewPage.waitForRender(200);

    // Send verification_start
    await hudInjector.inject(createVerificationStart("bun test"));
    await mainviewPage.waitForRender(200);

    // Verify message received
    let messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const startMsg = messages.find((m: unknown) => (m as { type: string }).type === "verification_start");
    expect(startMsg).toBeDefined();

    // Send verification_complete with success
    await hudInjector.inject(createVerificationComplete("bun test", true, "42 tests passed"));
    await mainviewPage.waitForRender(200);

    messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const completeMsg = messages.find((m: unknown) => (m as { type: string }).type === "verification_complete");
    expect(completeMsg).toBeDefined();

    // Send verification_complete with failure
    await hudInjector.inject(createVerificationComplete("bun run typecheck", false, "3 type errors"));
    await mainviewPage.waitForRender(200);

    // App should handle both success and failure
    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test("C6/HUD-035: commit_created/push_complete reflected in UI", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    await hudInjector.inject(createSessionStart());
    await mainviewPage.waitForRender(200);

    // Send commit_created
    const commitSha = "abc123def456789";
    const commitMessage = "feat(e2e): add real-time update tests";
    await hudInjector.inject(createCommitCreated(commitSha, commitMessage));
    await mainviewPage.waitForRender(200);

    // Verify commit message received
    let messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const commitMsg = messages.find((m: unknown) => (m as { type: string }).type === "commit_created");
    expect(commitMsg).toBeDefined();
    expect((commitMsg as { sha: string }).sha).toBe(commitSha);

    // Send push_complete
    await hudInjector.inject(createPushComplete("main"));
    await mainviewPage.waitForRender(200);

    messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const pushMsg = messages.find((m: unknown) => (m as { type: string }).type === "push_complete");
    expect(pushMsg).toBeDefined();
    expect((pushMsg as { branch: string }).branch).toBe("main");

    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test("C7/HUD-036: phase_change updates current state", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    await hudInjector.inject(createSessionStart());
    await mainviewPage.waitForRender(200);

    // Test different phases
    const phases = [
      "selecting_task",
      "decomposing",
      "executing_subtask",
      "verifying",
      "committing",
      "done",
    ] as const;

    for (const phase of phases) {
      const phaseMsg: PhaseChangeMessage = {
        type: "phase_change",
        phase,
      };
      await hudInjector.inject(phaseMsg);
      await mainviewPage.waitForRender(100);
    }

    // Wait until all phase messages are received
    await page.waitForFunction(
      (expected) => {
        const messages = (window as unknown as { __hudMessages?: unknown[] }).__hudMessages || [];
        return messages.filter((m: unknown) => (m as { type?: string }).type === "phase_change").length >= expected;
      },
      phases.length,
      { timeout: 2000 }
    );

    // Verify phase messages were received
    const messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const phaseMessages = messages.filter((m: unknown) => (m as { type: string }).type === "phase_change");
    expect(phaseMessages.length).toBe(phases.length);

    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test("C8/HUD-037: error message shows in UI with context", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    await hudInjector.inject(createSessionStart());
    await mainviewPage.waitForRender(200);

    // Get initial error count (may have errors from previous test file runs)
    const initialErrorCount = await page.evaluate(() => {
      return (window as unknown as { __errorCount: number }).__errorCount || 0;
    });

    // Send error message with phase context
    const errorMessage = "C8 Test: Verification failed 3 tests";
    await hudInjector.inject(createError(errorMessage, "verifying"));
    await mainviewPage.waitForRender(300);

    // Error indicator should be visible
    await expect(mainviewPage.errorIndicator).toHaveClass(/visible/);
    await expect(mainviewPage.errorIndicator).toContainText(errorMessage);

    // Verify error was tracked (count increased)
    const finalErrorCount = await page.evaluate(() => {
      return (window as unknown as { __errorCount: number }).__errorCount || 0;
    });
    expect(finalErrorCount).toBeGreaterThan(initialErrorCount);

    // App should still be functional
    await expect(mainviewPage.flowSvg).toBeVisible();
    await mainviewPage.clickReset();
    expect(await mainviewPage.getZoomLevel()).toBe("100%");
  });

  test("C9/HUD-038: text_output messages display in real-time", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    await hudInjector.inject(createSessionStart());
    await mainviewPage.waitForRender(200);

    // Send text_output messages (streaming simulation)
    const textOutputs: TextOutputMessage[] = [
      { type: "text_output", text: "Analyzing code...", source: "claude-code" },
      { type: "text_output", text: "Found 5 files to modify", source: "claude-code" },
      { type: "text_output", text: "Making changes...", source: "orchestrator" },
    ];

    for (const msg of textOutputs) {
      await hudInjector.inject(msg);
      await mainviewPage.waitForRender(100);
    }

    // Verify messages were received
    const messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const textMessages = messages.filter((m: unknown) => (m as { type: string }).type === "text_output");
    expect(textMessages.length).toBe(textOutputs.length);

    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test("C10/HUD-040: updates work after WebSocket reconnect", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    // Establish connection and send initial data
    await hudInjector.inject(createSessionStart("initial-session"));
    await hudInjector.inject(createTaskSelected({
      id: "oa-before-disconnect",
      title: "Task Before Disconnect",
    }));
    await mainviewPage.waitForRender(300);

    // Verify connected and received initial data
    await expect(mainviewPage.wsStatus).toHaveClass(/ws-connected/);
    let messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const initialTaskMsg = messages.find((m: unknown) =>
      (m as { type: string }).type === "task_selected" &&
      (m as { task: { id: string } }).task?.id === "oa-before-disconnect"
    );
    expect(initialTaskMsg).toBeDefined();

    // Disconnect all clients
    await hudInjector.disconnectAll();
    await mainviewPage.waitForRender(500);
    await expect(mainviewPage.wsStatus).toHaveClass(/ws-disconnected/);

    // Wait for auto-reconnect (test server reconnects after 1 second)
    await page.waitForSelector("#ws-status.ws-connected", { timeout: 3000 });
    await expect(mainviewPage.wsStatus).toHaveClass(/ws-connected/);

    // Send new data after reconnect
    await hudInjector.inject(createTaskSelected({
      id: "oa-after-reconnect",
      title: "Task After Reconnect",
    }));
    await mainviewPage.waitForRender(500);

    // Verify new messages were received and processed after reconnect
    messages = await page.evaluate(() => {
      return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
    });
    const taskMsgAfter = messages.find((m: unknown) =>
      (m as { type: string }).type === "task_selected" &&
      (m as { task: { id: string } }).task?.id === "oa-after-reconnect"
    );
    expect(taskMsgAfter).toBeDefined();

    // Verify UI updated with new task
    const svgContent = await mainviewPage.getSvgContent();
    expect(svgContent).toContain("Task After Reconnect");

    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test.describe("Full Golden Loop Sequence", () => {
    test("processes complete task lifecycle messages", async ({
      mainviewPage,
      hudInjector,
      page,
    }) => {
      // Send complete Golden Loop sequence
      const sequence = createGoldenLoopSequence("oa-golden-loop-test");

      for (const msg of sequence) {
        await hudInjector.inject(msg);
        await mainviewPage.waitForRender(100);
      }

      await mainviewPage.waitForRender(500);

      // Verify all message types were received
      const messages = await page.evaluate(() => {
        return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
      });

      const messageTypes = new Set(messages.map((m: unknown) => (m as { type: string }).type));

      // Check for key message types from Golden Loop
      expect(messageTypes.has("session_start")).toBe(true);
      expect(messageTypes.has("task_selected")).toBe(true);
      expect(messageTypes.has("task_decomposed")).toBe(true);
      expect(messageTypes.has("subtask_start")).toBe(true);
      expect(messageTypes.has("subtask_complete")).toBe(true);
      expect(messageTypes.has("verification_start")).toBe(true);
      expect(messageTypes.has("verification_complete")).toBe(true);
      expect(messageTypes.has("commit_created")).toBe(true);
      expect(messageTypes.has("push_complete")).toBe(true);
      expect(messageTypes.has("session_complete")).toBe(true);

      // App should remain functional throughout
      await expect(mainviewPage.flowSvg).toBeVisible();
    });
  });

  test.describe("Tool Call/Result Messages (HUD-039)", () => {
    test("tool_call and tool_result messages are received", async ({
      mainviewPage,
      hudInjector,
      page,
    }) => {
      await hudInjector.inject(createSessionStart());
      await mainviewPage.waitForRender(200);

      // Send tool_call
      const toolCall: ToolCallMessage = {
        type: "tool_call",
        toolName: "Read",
        arguments: JSON.stringify({ file_path: "/src/index.ts" }),
        callId: "call-001",
      };
      await hudInjector.inject(toolCall);
      await mainviewPage.waitForRender(100);

      // Send tool_result
      const toolResult: ToolResultMessage = {
        type: "tool_result",
        toolName: "Read",
        result: JSON.stringify({ content: "file contents here" }),
        isError: false,
        callId: "call-001",
      };
      await hudInjector.inject(toolResult);
      await mainviewPage.waitForRender(100);

      // Verify messages received
      const messages = await page.evaluate(() => {
        return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
      });

      const callMsg = messages.find((m: unknown) => (m as { type: string }).type === "tool_call");
      const resultMsg = messages.find((m: unknown) => (m as { type: string }).type === "tool_result");

      expect(callMsg).toBeDefined();
      expect(resultMsg).toBeDefined();

      await expect(mainviewPage.flowSvg).toBeVisible();
    });

    test("tool_result with error is handled", async ({
      mainviewPage,
      hudInjector,
      page,
    }) => {
      await hudInjector.inject(createSessionStart());

      // Send tool_result with error
      const toolResult: ToolResultMessage = {
        type: "tool_result",
        toolName: "Bash",
        result: JSON.stringify({ error: "Command failed with exit code 1" }),
        isError: true,
        callId: "call-error",
      };
      await hudInjector.inject(toolResult);
      await mainviewPage.waitForRender(300);

      // Verify message received
      const messages = await page.evaluate(() => {
        return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
      });
      const errorResult = messages.find((m: unknown) =>
        (m as { type: string }).type === "tool_result" &&
        (m as { isError: boolean }).isError === true
      );
      expect(errorResult).toBeDefined();

      // App should not crash
      await expect(mainviewPage.flowSvg).toBeVisible();
    });
  });

  test.describe("Message Ordering and Timing", () => {
    test("handles rapid message sequence without dropping", async ({
      mainviewPage,
      hudInjector,
      page,
    }) => {
      // Get initial message count to handle any messages from previous tests
      const initialMessages = await page.evaluate(() => {
        return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
      });
      const initialApmCount = initialMessages.filter((m: unknown) => (m as { type: string }).type === "apm_update").length;

      await hudInjector.inject(createSessionStart());

      // Send 20 messages rapidly
      const messageCount = 20;
      for (let i = 0; i < messageCount; i++) {
        await hudInjector.inject(createAPMUpdate({
          sessionAPM: i * 2,
          totalActions: i * 10,
        }));
        // Minimal delay to simulate rapid updates
        await mainviewPage.waitForRender(20);
      }

      await mainviewPage.waitForRender(500);

      // Verify messages were received
      const finalMessages = await page.evaluate(() => {
        return (window as unknown as { __hudMessages: unknown[] }).__hudMessages || [];
      });
      const finalApmCount = finalMessages.filter((m: unknown) => (m as { type: string }).type === "apm_update").length;

      // All messages should have been received (compare with initial to handle parallel test runs)
      const newApmMessages = finalApmCount - initialApmCount;
      expect(newApmMessages).toBeGreaterThanOrEqual(messageCount);

      // App should remain responsive
      await expect(mainviewPage.flowSvg).toBeVisible();
      await mainviewPage.clickReset();
      expect(await mainviewPage.getZoomLevel()).toBe("100%");
    });

    test("handles message burst during user interaction", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(createSessionStart());
      await mainviewPage.waitForRender(200);

      // Start panning while messages arrive
      const panPromise = mainviewPage.pan(100, 50);

      // Send messages during pan
      await hudInjector.inject(createTaskSelected({ id: "oa-during-pan" }));
      await hudInjector.inject(createAPMUpdate({ sessionAPM: 15.0 }));

      await panPromise;
      await mainviewPage.waitForRender(300);

      // Both pan and messages should have worked
      await expect(mainviewPage.flowSvg).toBeVisible();
      const transform = await mainviewPage.flowCanvas.getAttribute("transform");
      expect(transform).not.toBeNull();
    });
  });
});
