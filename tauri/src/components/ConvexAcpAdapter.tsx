/**
 * Adapter component that listens for ACP events from Tauri backend
 * and writes them to Convex instead of Tinyvex
 *
 * This is Option A from CONVEX_MIGRATION.md:
 * Rust emits Tauri events → Frontend writes to Convex
 */

import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useMutation } from "convex/react";
import { api } from "../../../convex/_generated/api";
import { FEATURE_FLAGS } from "@/lib/feature-flags";
import type { Id } from "../../../convex/_generated/dataModel";

interface AcpMessageEvent {
  threadId: string;
  itemId: string;
  role: "user" | "assistant" | "system";
  content: string;
  kind?: "message" | "reason";
  partial?: boolean;
  seq?: number;
}

interface AcpToolCallEvent {
  threadId: string;
  toolCallId: string;
  title?: string;
  kind?: string;
  status?: string;
  contentJson?: string;
  locationsJson?: string;
}

interface AcpPlanEvent {
  threadId: string;
  entriesJson: string;
}

interface AcpStateEvent {
  threadId: string;
  currentModeId?: string;
  availableCommandsJson?: string;
}

interface AcpEventLogEvent {
  sessionId?: string;
  clientThreadDocId?: string;
  threadId?: string;
  updateKind?: string;
  payload: string;
}

export function ConvexAcpAdapter() {
  const upsertStreamingMessage = useMutation(api.chat.upsertStreamingMessage);
  const finalizeMessage = useMutation(api.chat.finalizeMessage);
  const upsertToolCall = useMutation(api.toolCalls.upsertToolCall);
  const upsertPlan = useMutation(api.planEntries.upsertPlan);
  const upsertThreadState = useMutation(api.threadState.upsertThreadState);
  const appendEvent = useMutation(api.acpEvents.appendEvent);

  useEffect(() => {
    // Only listen if Convex is enabled
    if (!FEATURE_FLAGS.CONVEX_MESSAGES) {
      return;
    }

    const listeners: (() => void)[] = [];

    // Listen for message events
    listen<AcpMessageEvent>("acp:message", async (event) => {
      try {
        await upsertStreamingMessage({
          threadId: event.payload.threadId as Id<"threads">,
          itemId: event.payload.itemId,
          role: event.payload.role,
          content: event.payload.content,
          kind: event.payload.kind,
          partial: event.payload.partial,
          seq: event.payload.seq,
        });
      } catch (error) {
        console.error("[ConvexAcpAdapter] Failed to upsert message:", error);
      }
    }).then((unlisten) => listeners.push(unlisten));

    // Listen for message finalization events
    listen<{ itemId: string }>("acp:message:finalize", async (event) => {
      try {
        await finalizeMessage({ itemId: event.payload.itemId });
      } catch (error) {
        console.error("[ConvexAcpAdapter] Failed to finalize message:", error);
      }
    }).then((unlisten) => listeners.push(unlisten));

    // Listen for tool call events
    listen<AcpToolCallEvent>("acp:tool_call", async (event) => {
      try {
        await upsertToolCall({
          threadId: event.payload.threadId as Id<"threads">,
          toolCallId: event.payload.toolCallId,
          title: event.payload.title,
          kind: event.payload.kind,
          status: event.payload.status,
          contentJson: event.payload.contentJson,
          locationsJson: event.payload.locationsJson,
        });
      } catch (error) {
        console.error("[ConvexAcpAdapter] Failed to upsert tool call:", error);
      }
    }).then((unlisten) => listeners.push(unlisten));

    // Listen for plan events
    listen<AcpPlanEvent>("acp:plan", async (event) => {
      try {
        await upsertPlan({
          threadId: event.payload.threadId as Id<"threads">,
          entriesJson: event.payload.entriesJson,
        });
      } catch (error) {
        console.error("[ConvexAcpAdapter] Failed to upsert plan:", error);
      }
    }).then((unlisten) => listeners.push(unlisten));

    // Listen for state events
    listen<AcpStateEvent>("acp:state", async (event) => {
      try {
        await upsertThreadState({
          threadId: event.payload.threadId as Id<"threads">,
          currentModeId: event.payload.currentModeId,
          availableCommandsJson: event.payload.availableCommandsJson,
        });
      } catch (error) {
        console.error("[ConvexAcpAdapter] Failed to upsert thread state:", error);
      }
    }).then((unlisten) => listeners.push(unlisten));

    // Listen for event log entries
    listen<AcpEventLogEvent>("acp:event", async (event) => {
      try {
        await appendEvent({
          sessionId: event.payload.sessionId,
          clientThreadDocId: event.payload.clientThreadDocId,
          threadId: event.payload.threadId as Id<"threads"> | undefined,
          updateKind: event.payload.updateKind,
          payload: event.payload.payload,
        });
      } catch (error) {
        console.error("[ConvexAcpAdapter] Failed to append event:", error);
      }
    }).then((unlisten) => listeners.push(unlisten));

    console.info("[ConvexAcpAdapter] Listening for ACP events from Tauri backend");

    // Cleanup listeners on unmount
    return () => {
      listeners.forEach((unlisten) => unlisten());
      console.info("[ConvexAcpAdapter] Stopped listening for ACP events");
    };
  }, [
    upsertStreamingMessage,
    finalizeMessage,
    upsertToolCall,
    upsertPlan,
    upsertThreadState,
    appendEvent,
  ]);

  // This is an invisible component that just listens for events
  return null;
}
