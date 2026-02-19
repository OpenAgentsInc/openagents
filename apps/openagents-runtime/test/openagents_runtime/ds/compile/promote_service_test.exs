defmodule OpenAgentsRuntime.DS.Compile.PromoteServiceTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.DS.Compile.PromoteService
  alias OpenAgentsRuntime.DS.PolicyRegistry

  @signature_id "@openagents/autopilot/rlm/SummarizeThread.v1"

  test "promote/3 promotes selected artifact and writes audit log" do
    assert {:ok, _pointer} =
             PolicyRegistry.upsert_pointer(@signature_id, %{
               primary_artifact: %{compiled_id: "cmp_primary_v1", strategy_id: "direct.v1"},
               canary_artifact: %{compiled_id: "cmp_canary_v1", strategy_id: "rlm_lite.v1"},
               canary_percent: 40
             })

    assert {:ok, result} =
             PromoteService.promote(@signature_id, "cmp_canary_v1",
               actor: "ops_user",
               reason: "canary looks good"
             )

    assert result.action == "promote"
    assert result.primary_artifact["compiled_id"] == "cmp_canary_v1"
    assert result.canary_artifact == nil
    assert result.canary_percent == 0
    assert is_integer(result.audit_id)

    [audit] = PromoteService.list_audits(@signature_id)
    assert audit.action == "promote"
    assert audit.actor == "ops_user"
    assert audit.before_pointer["primary_artifact"]["compiled_id"] == "cmp_primary_v1"
    assert audit.after_pointer["primary_artifact"]["compiled_id"] == "cmp_canary_v1"
  end

  test "rollback/2 restores previous pointer state and records rollback audit" do
    assert {:ok, _pointer} =
             PolicyRegistry.upsert_pointer(@signature_id, %{
               primary_artifact: %{compiled_id: "cmp_primary_v2", strategy_id: "direct.v1"},
               canary_artifact: %{compiled_id: "cmp_canary_v2", strategy_id: "rlm_lite.v1"},
               canary_percent: 50,
               rollout_seed: "seed_rollback"
             })

    assert {:ok, promoted} = PromoteService.promote(@signature_id, "cmp_canary_v2")

    assert {:ok, rolled_back} =
             PromoteService.rollback(@signature_id, actor: "ops_user", reason: "rollback check")

    assert rolled_back.action == "rollback"
    assert rolled_back.primary_artifact["compiled_id"] == "cmp_primary_v2"
    assert rolled_back.canary_artifact["compiled_id"] == "cmp_canary_v2"
    assert rolled_back.canary_percent == 50

    [latest, previous] = PromoteService.list_audits(@signature_id, limit: 2)
    assert latest.action == "rollback"
    assert latest.target_audit_id == promoted.audit_id
    assert previous.action == "promote"
  end

  test "promote/3 returns pointer_not_found when signature pointer is missing" do
    assert {:error, :pointer_not_found} =
             PromoteService.promote("missing.signature.v1", "cmp_any")
  end

  test "rollback/2 returns rollback_unavailable when no prior audit exists" do
    assert {:ok, _pointer} =
             PolicyRegistry.upsert_pointer(@signature_id, %{
               primary_artifact: %{compiled_id: "cmp_primary_v3", strategy_id: "direct.v1"},
               canary_percent: 0
             })

    assert {:error, :rollback_unavailable} = PromoteService.rollback(@signature_id)
  end
end
