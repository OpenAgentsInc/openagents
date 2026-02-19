defmodule OpenAgentsRuntime.DS.Strategies.RlmLiteV1Test do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.DS.Strategies.RlmLiteV1
  alias OpenAgentsRuntime.DS.Traces

  test "execute/3 runs bounded iterations and injects replay summaries" do
    signature = %{
      signature_id: "@openagents/autopilot/rlm/SummarizeThread.v1",
      name: "SummarizeThread"
    }

    replay_tasks =
      Enum.map(1..30, fn index ->
        %{
          tool_call_id: "call_#{index}",
          tool_name: "test.tool",
          state: "succeeded",
          output: %{"text" => String.duplicate("x", 120), "index" => index},
          queued_at: "2026-02-19T02:00:00Z"
        }
      end)

    assert {:ok, result} =
             RlmLiteV1.execute(
               signature,
               %{"timeline_window" => %{"from" => "a", "to" => "b"}},
               run_id: "run_rlm_test",
               max_iterations: 4,
               tool_replay_tasks: replay_tasks,
               max_replay_items: 8,
               max_replay_total_chars: 700
             )

    assert result.output["confidence"] >= 0.45
    assert result.trace["trace_ref"] =~ "trace:run_rlm_test:"
    assert result.replay["window"]["included_items"] <= 8
    assert result.replay["window"]["truncated_items"] > 0
  end

  test "execute/3 rejects invalid iteration budgets" do
    assert {:error, :invalid_iteration_budget} =
             RlmLiteV1.execute(%{}, %{}, max_iterations: 0)

    assert {:error, :invalid_iteration_budget} =
             RlmLiteV1.execute(%{}, %{}, max_iterations: 99)
  end

  test "trace capture offloads large payloads to pointer storage metadata" do
    payload = %{
      "strategy_id" => "rlm_lite.v1",
      "iterations" =>
        Enum.map(1..20, &%{"iteration" => &1, "notes" => String.duplicate("n", 250)})
    }

    trace =
      Traces.capture(
        "run_rlm_blob",
        "@openagents/autopilot/rlm/SummarizeThread.v1",
        payload,
        max_inline_bytes: 300,
        uri_prefix: "gcs://runtime-test-traces"
      )

    assert trace["storage"] == "external"
    assert trace["trace_ref"] =~ "trace:run_rlm_blob:"
    assert trace["artifact_uri"] =~ "gcs://runtime-test-traces/run_rlm_blob/"
    assert trace["offloaded_bytes"] > 300
  end
end
