defmodule OpenAgentsRuntime.DS.Compile.CompileServiceTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.DS.Compile.CompileService

  @signature_id "@openagents/autopilot/blueprint/SelectTool.v1"

  test "compile/2 persists compile and eval reports deterministically" do
    receipts = sample_receipts(12)
    traces = sample_traces(12)

    opts = [
      receipts: receipts,
      traces: traces,
      dataset_opts: [
        split: %{train: 60, holdout: 20, test: 20},
        split_seed: "compile_seed",
        generated_at: "2026-02-19T04:00:00Z"
      ],
      search_space: [
        %{"compiled_id" => "cmp_selecttool_direct", "strategy_id" => "direct.v1"},
        %{"compiled_id" => "cmp_selecttool_rlm", "strategy_id" => "rlm_lite.v1"}
      ],
      started_at: ~U[2026-02-19 04:00:00Z]
    ]

    assert {:ok, report} = CompileService.compile(@signature_id, opts)
    assert report.signature_id == @signature_id
    assert report.status == "succeeded"
    assert report.compiled_id in ["cmp_selecttool_direct", "cmp_selecttool_rlm"]
    assert String.length(report.job_hash) == 64
    assert String.length(report.dataset_hash) == 64
    assert length(report.eval_reports) == 6

    persisted = CompileService.get_report(report.report_id)
    assert persisted.report_id == report.report_id
    assert persisted.job_hash == report.job_hash
    assert length(persisted.eval_reports) == 6
  end

  test "compile/2 returns idempotent replay on duplicate job spec and dataset hash" do
    receipts = sample_receipts(8)

    opts = [
      receipts: receipts,
      dataset_opts: [split_seed: "stable_seed", generated_at: "2026-02-19T05:00:00Z"]
    ]

    assert {:ok, first} = CompileService.compile(@signature_id, opts)
    refute first.idempotent_replay

    assert {:ok, second} = CompileService.compile(@signature_id, opts)
    assert second.idempotent_replay
    assert second.report_id == first.report_id
    assert second.job_hash == first.job_hash

    reports = CompileService.list_reports(@signature_id)
    assert length(reports) == 1
  end

  test "compile/2 fails on invalid search space" do
    assert {:error, :invalid_search_space} =
             CompileService.compile(@signature_id,
               search_space: [%{"strategy_id" => "direct.v1"}]
             )
  end

  test "compile/2 fails when dataset export has no valid examples" do
    invalid_receipts = [%{receipt_id: "bad", signature_id: @signature_id}]

    assert {:error, :empty_dataset} =
             CompileService.compile(@signature_id, receipts: invalid_receipts)
  end

  defp sample_receipts(count) do
    Enum.map(1..count, fn index ->
      %{
        receipt_id: "pred_compile_#{index}",
        signature_id: @signature_id,
        strategy_id: "direct.v1",
        compiled_id: "cmp_seed_v1",
        params_hash: "params_#{index}",
        output_hash: "output_#{index}",
        trace_ref: "trace:run_compile:#{index}",
        trace_hash: "trace_hash_#{index}",
        policy: %{decision: "allowed"},
        budget: %{spent_sats: index},
        timing: %{latency_ms: 20 + index}
      }
    end)
  end

  defp sample_traces(count) do
    Enum.map(1..count, fn index ->
      %{
        trace_ref: "trace:run_compile:#{index}",
        trace_hash: "trace_hash_#{index}",
        storage: "inline",
        payload: %{summary: "trace_#{index}"}
      }
    end)
  end
end
