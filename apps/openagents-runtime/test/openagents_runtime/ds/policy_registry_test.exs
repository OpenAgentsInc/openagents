defmodule OpenAgentsRuntime.DS.PolicyRegistryTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.DS.PolicyRegistry
  alias OpenAgentsRuntime.DS.Predict

  @signature_id "@openagents/autopilot/rlm/SummarizeThread.v1"

  test "upsert_pointer stores primary/canary artifacts and selects deterministically" do
    assert {:ok, _pointer} =
             PolicyRegistry.upsert_pointer(@signature_id, %{
               primary_artifact: %{compiled_id: "cmp_primary_v1", strategy_id: "direct.v1"},
               canary_artifact: %{compiled_id: "cmp_canary_v1", strategy_id: "rlm_lite.v1"},
               canary_percent: 30,
               rollout_seed: "seed-alpha"
             })

    assert PolicyRegistry.pointer_count() >= 1

    assert {:ok, artifact_a} = PolicyRegistry.active_artifact(@signature_id, run_id: "run_123")
    assert {:ok, artifact_b} = PolicyRegistry.active_artifact(@signature_id, run_id: "run_123")
    assert artifact_a == artifact_b
    assert artifact_a["compiled_id"] in ["cmp_primary_v1", "cmp_canary_v1"]
    assert artifact_a["variant"] in ["primary", "canary"]
  end

  test "canary percent gates selected variant and clear_canary resets to primary" do
    assert {:ok, _pointer} =
             PolicyRegistry.upsert_pointer(@signature_id, %{
               primary_artifact: %{compiled_id: "cmp_primary_v2", strategy_id: "direct.v1"},
               canary_artifact: %{compiled_id: "cmp_canary_v2", strategy_id: "rlm_lite.v1"},
               canary_percent: 100,
               rollout_seed: "seed-beta"
             })

    assert {:ok, artifact} = PolicyRegistry.active_artifact(@signature_id, canary_key: "always")
    assert artifact["variant"] == "canary"
    assert artifact["strategy_id"] == "rlm_lite.v1"

    assert {:ok, _} = PolicyRegistry.clear_canary(@signature_id)

    assert {:ok, cleared_artifact} =
             PolicyRegistry.active_artifact(@signature_id, canary_key: "always")

    assert cleared_artifact["variant"] == "primary"
    assert cleared_artifact["compiled_id"] == "cmp_primary_v2"
    assert cleared_artifact["strategy_id"] == "direct.v1"
  end

  test "upsert_pointer enforces canary artifact when canary percent is enabled" do
    assert {:error, changeset} =
             PolicyRegistry.upsert_pointer(@signature_id, %{
               primary_artifact: %{compiled_id: "cmp_primary_v3", strategy_id: "direct.v1"},
               canary_percent: 10
             })

    assert %{canary_artifact: ["is required when canary_percent > 0"]} = errors_on(changeset)
  end

  test "predict uses pointer-selected canary strategy when not explicitly overridden" do
    assert {:ok, _pointer} =
             PolicyRegistry.upsert_pointer(@signature_id, %{
               primary_artifact: %{compiled_id: "cmp_primary_predict", strategy_id: "direct.v1"},
               canary_artifact: %{compiled_id: "cmp_canary_predict", strategy_id: "rlm_lite.v1"},
               canary_percent: 100,
               rollout_seed: "seed-predict"
             })

    input = %{
      "timeline_window" => %{"from" => "2026-02-18T00:00:00Z", "to" => "2026-02-19T00:00:00Z"},
      "tool_replay" => %{
        "summary" => "tool_a|succeeded|ok",
        "trace_refs" => ["tool_task:tool_a"],
        "window" => %{"included_items" => 1}
      }
    }

    assert {:ok, result} = Predict.run(@signature_id, input, run_id: "run_canary_predict")

    assert result.receipt.strategy_id == "rlm_lite.v1"
    assert result.receipt.compiled_id == "cmp_canary_predict"
    assert result.receipt.policy["artifact_variant"] == "canary"
    assert result.receipt.policy["canary_percent"] == 100
  end
end
