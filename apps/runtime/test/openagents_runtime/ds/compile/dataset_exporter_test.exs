defmodule OpenAgentsRuntime.DS.Compile.DatasetExporterTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.DS.Compile.DatasetExporter

  test "export/3 builds deterministic split datasets and hashes" do
    receipts =
      Enum.map(1..12, fn index ->
        %{
          receipt_id: "pred_#{index}",
          signature_id: "@openagents/autopilot/blueprint/SelectTool.v1",
          strategy_id: "direct.v1",
          compiled_id: "cmp_v1",
          params_hash: "params_#{index}",
          output_hash: "output_#{index}",
          trace_ref: "trace:run_1:#{index}",
          trace_hash: "trace_hash_#{index}",
          policy: %{decision: "allowed"},
          budget: %{spent_sats: index},
          timing: %{latency_ms: index * 10}
        }
      end)

    traces =
      Enum.map(1..12, fn index ->
        %{
          trace_ref: "trace:run_1:#{index}",
          trace_hash: "trace_hash_#{index}",
          storage: "inline",
          payload: %{summary: "trace_#{index}"}
        }
      end)

    opts = [
      split: %{train: 70, holdout: 20, test: 10},
      split_seed: "seed_dataset",
      dataset_name: "ds_export_test",
      generated_at: "2026-02-19T03:00:00Z"
    ]

    assert {:ok, export_a} = DatasetExporter.export(receipts, traces, opts)
    assert {:ok, export_b} = DatasetExporter.export(receipts, traces, opts)

    assert export_a.dataset_hash == export_b.dataset_hash
    assert export_a.job_hash == export_b.job_hash
    assert export_a.splits == export_b.splits
    assert export_a.counts.total == 12
    assert export_a.counts.skipped == 0
    assert export_a.counts.train + export_a.counts.holdout + export_a.counts.test == 12
  end

  test "export/3 links trace provenance into examples" do
    receipt = %{
      receipt_id: "pred_trace_link",
      signature_id: "@openagents/autopilot/rlm/SummarizeThread.v1",
      strategy_id: "rlm_lite.v1",
      compiled_id: "cmp_rlm_v1",
      params_hash: "params_hash",
      output_hash: "output_hash",
      trace_ref: "trace:run_x:abc123",
      policy: %{decision: "allowed"},
      budget: %{spent_sats: 2},
      timing: %{latency_ms: 50}
    }

    trace = %{
      trace_ref: "trace:run_x:abc123",
      trace_hash: "trace_hash_abc123",
      storage: "external",
      artifact_uri: "gcs://runtime-test/trace.json",
      payload: %{"summary" => "trace summary"}
    }

    assert {:ok, export} =
             DatasetExporter.export([receipt], [trace], split: %{train: 100, holdout: 0, test: 0})

    assert [example] = export.splits.train
    assert example["trace_ref"] == "trace:run_x:abc123"
    assert example["trace_hash"] == "trace_hash_abc123"
    assert example["trace_storage"] == "external"
    assert example["trace_artifact_uri"] == "gcs://runtime-test/trace.json"
    assert example["trace_summary"] == "trace summary"
  end

  test "export/3 rejects invalid split definitions" do
    assert {:error, :invalid_split} =
             DatasetExporter.export([], [], split: %{train: 60, holdout: 20, test: 10})
  end

  test "export/3 skips receipts missing required fields" do
    invalid_receipt = %{receipt_id: "pred_invalid", signature_id: "sig_only"}

    assert {:ok, export} = DatasetExporter.export([invalid_receipt], [])
    assert export.counts.total == 0
    assert export.counts.skipped == 1
    assert [%{reason: {:missing_fields, fields}}] = export.skipped
    assert "params_hash" in fields
    assert "output_hash" in fields
  end
end
