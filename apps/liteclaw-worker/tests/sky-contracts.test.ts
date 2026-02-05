import { describe, expect, it } from "vitest";
import {
  SKY_EVENT_PAYLOAD_SCHEMAS,
  createSkyValidators,
  type SkyEventType
} from "../src/sky/contracts";

describe("Sky contracts", () => {
  const validators = createSkyValidators();

  it("validates message schema", () => {
    const message = {
      id: "msg_1",
      role: "user",
      parts: [{ type: "text", text: "Hello" }]
    };

    expect(validators.validateMessage(message)).toBe(true);
  });

  it("validates receipt schema", () => {
    const runReceipt = {
      schema_version: 1,
      cf_sky_version: "0.1.0",
      type: "run",
      run_id: "run_1",
      thread_id: "thread_1",
      model_config_id: "workers-ai:llama-3.1-8b-instruct",
      input_hash: "inputhash",
      output_hash: "outputhash",
      started_at: 1,
      completed_at: 2,
      duration_ms: 1,
      status: "completed",
      finish_reason: "stop",
      error_code: null
    };

    const toolReceipt = {
      schema_version: 1,
      cf_sky_version: "0.1.0",
      type: "tool",
      run_id: "run_1",
      thread_id: "thread_1",
      tool_call_id: "tool_1",
      tool_name: "http.fetch",
      args_hash: "argshash",
      output_hash: "outputhash",
      patch_hash: null,
      local_receipt: {
        tool_name: "http.fetch",
        args_hash: "argshash",
        output_hash: "outputhash",
        patch_hash: null,
        executor_kind: "tunnel",
        started_at: 1,
        completed_at: 2,
        duration_ms: 1
      },
      local_signature: "sig",
      started_at: 1,
      completed_at: 2,
      duration_ms: 1,
      status: "success",
      error_code: null
    };

    expect(validators.validateReceipt(runReceipt)).toBe(true);
    expect(validators.validateReceipt(toolReceipt)).toBe(true);
  });

  it("validates every event payload schema", () => {
    const payloads: Record<SkyEventType, unknown> = {
      "run.started": {
        thread_id: "thread_1",
        model_config_id: "workers-ai:llama-3.1-8b-instruct",
        started_at: 1,
        schema_version: 1
      },
      "model.delta": {
        kind: "text-delta",
        delta: "Hel"
      },
      "model.completed": {
        finish_reason: "stop",
        text_length: 5
      },
      "run.error": {
        error: "boom"
      },
      "run.completed": {
        status: "completed",
        finish_reason: "stop",
        duration_ms: 10
      },
      "tool.call.started": {
        tool_call_id: "tool_1",
        tool_name: "http.fetch"
      },
      "tool.call.args.delta": {
        tool_call_id: "tool_1",
        tool_name: "http.fetch",
        delta: "{\"url\":",
        format: "json"
      },
      "tool.call.args.completed": {
        tool_call_id: "tool_1",
        tool_name: "http.fetch",
        args: { url: "https://example.com" }
      },
      "tool.call.completed": {
        tool_call_id: "tool_1",
        tool_name: "http.fetch",
        status: "success",
        duration_ms: 15
      },
      "tool.result": {
        tool_call_id: "tool_1",
        tool_name: "http.fetch",
        status: "success",
        output_hash: "hash"
      }
    };

    const eventTypes = Object.keys(
      SKY_EVENT_PAYLOAD_SCHEMAS
    ) as Array<SkyEventType>;

    for (const type of eventTypes) {
      const ok = validators.validateEventPayload(type, payloads[type]);
      expect(ok, `Event payload for ${type} failed validation`).toBe(true);
    }
  });
});
