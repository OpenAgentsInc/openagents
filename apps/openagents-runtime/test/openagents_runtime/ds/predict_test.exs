defmodule OpenAgentsRuntime.DS.PredictTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.DS.Predict
  alias OpenAgentsRuntime.DS.Signatures.Catalog

  @signature_id "@openagents/autopilot/blueprint/SelectTool.v1"

  test "run/3 executes direct.v1 predict and emits deterministic receipt fields" do
    started_at = ~U[2026-02-19 02:00:00Z]
    completed_at = ~U[2026-02-19 02:00:02Z]

    input = %{
      "messages" => [%{"role" => "user", "content" => "Find docs on memory rollups"}],
      "tools" => [%{"name" => "search.docs", "description" => "Search docs"}],
      "context" => %{"memory" => %{"l1" => "recent"}}
    }

    opts = [
      run_id: "run_ds_001",
      started_at: started_at,
      completed_at: completed_at,
      policy: %{
        policy_id: "policy_default",
        authorization_id: "auth_123",
        authorization_mode: "delegated_budget",
        decision: "allowed"
      },
      budget: %{spent_sats: 10, reserved_sats: 2, remaining_sats: 88}
    ]

    assert {:ok, result_a} = Predict.run(@signature_id, input, opts)
    assert {:ok, result_b} = Predict.run(@signature_id, input, opts)

    assert result_a == result_b
    assert result_a.output["tool_name"] == "search.docs"

    receipt = result_a.receipt
    assert receipt.run_id == "run_ds_001"
    assert receipt.signature_id == @signature_id
    assert receipt.strategy_id == "direct.v1"
    assert String.length(receipt.schema_hash) == 64
    assert String.length(receipt.prompt_hash) == 64
    assert String.length(receipt.program_hash) == 64
    assert String.length(receipt.params_hash) == 64
    assert String.length(receipt.output_hash) == 64

    assert receipt.policy["authorization_id"] == "auth_123"
    assert receipt.policy["authorization_mode"] == "delegated_budget"
    assert receipt.policy["decision"] == "allowed"

    assert receipt.budget["spent_sats"] == 10
    assert receipt.budget["reserved_sats"] == 2
    assert receipt.budget["remaining_sats"] == 88
    assert receipt.timing["latency_ms"] == 2_000
  end

  test "run/3 rejects unsupported strategy ids" do
    assert {:error, {:unsupported_strategy, "unknown.v9"}} =
             Predict.run(@signature_id, %{}, strategy_id: "unknown.v9")
  end

  test "run/3 rejects unknown signatures" do
    assert {:error, :signature_not_found} = Predict.run("missing.v1", %{})
  end

  test "run/3 enforces artifact compatibility checks" do
    assert {:ok, hashes} = Catalog.hashes(@signature_id)

    invalid_artifact = %{
      schema_hash: String.duplicate("0", 64),
      prompt_hash: hashes.prompt_hash,
      program_hash: hashes.program_hash
    }

    assert {:error, {:artifact_incompatible, {:hash_mismatch, :schema_hash}}} =
             Predict.run(@signature_id, %{}, artifact: invalid_artifact)
  end

  test "run/3 rejects invalid custom direct output" do
    assert {:error, :invalid_output} = Predict.run(@signature_id, %{}, output: 123)
  end

  test "run/3 supports rlm_lite.v1 with trace linkage in receipt" do
    signature_id = "@openagents/autopilot/rlm/SummarizeThread.v1"

    input = %{
      "timeline_window" => %{"from" => "2026-02-18T00:00:00Z", "to" => "2026-02-19T00:00:00Z"},
      "tool_replay" => %{
        "summary" => "tool_a|success|ok",
        "trace_refs" => ["tool_task:tool_a"],
        "window" => %{"included_items" => 1}
      }
    }

    assert {:ok, result} =
             Predict.run(
               signature_id,
               input,
               run_id: "run_rlm_001",
               strategy_id: "rlm_lite.v1",
               max_iterations: 3
             )

    assert result.receipt.strategy_id == "rlm_lite.v1"
    assert result.receipt.trace_ref =~ "trace:run_rlm_001:"
    assert is_binary(result.receipt.trace_hash)
    assert result.receipt.trace_storage in ["inline", "external"]
    assert result.trace["trace_ref"] == result.receipt.trace_ref
    assert result.output["confidence"] >= 0.45
  end
end
