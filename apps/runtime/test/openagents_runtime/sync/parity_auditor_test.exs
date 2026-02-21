defmodule OpenAgentsRuntime.Sync.ParityAuditorTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Khala.ProjectionCheckpoint
  alias OpenAgentsRuntime.Khala.Projector
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.CodexWorkerSummary
  alias OpenAgentsRuntime.Sync.ParityAuditor
  alias OpenAgentsRuntime.Sync.PayloadHash
  alias OpenAgentsRuntime.Sync.RunSummary

  @projected_at ~U[2026-02-20 00:00:00.000000Z]

  @tag :sync_parity
  test "run_once reports clean parity for matching summaries" do
    reset_parity_tables!()

    run_id = unique_id("run_parity_ok")
    document_id = "runtime/run_summary:#{run_id}"
    payload = run_payload(run_id, document_id, 7)

    insert_checkpoint("run_summary", run_id, document_id, 7, payload)
    insert_run_summary(document_id, 7, payload)

    summary =
      ParityAuditor.run_once(
        enabled: true,
        sample_size: 10,
        projection_names: ["run_summary"],
        emit_entity_events: false,
        emit_parity_failures: false
      )

    assert summary.sampled == 1
    assert summary.mismatches == 0
    assert summary.hash_mismatches == 0
    assert summary.lag_drift_nonzero == 0
    assert summary.mismatch_rate == 0.0
    assert summary.status == :ok
  end

  @tag :sync_parity
  test "run_once reports hash mismatch and lag drift" do
    reset_parity_tables!()

    run_id = unique_id("run_parity_drift")
    document_id = "runtime/run_summary:#{run_id}"
    payload = run_payload(run_id, document_id, 9)
    drifted_payload = Map.put(payload, "status", "failed")

    insert_checkpoint("run_summary", run_id, document_id, 9, payload)
    insert_run_summary(document_id, 8, drifted_payload)

    summary =
      ParityAuditor.run_once(
        enabled: true,
        sample_size: 10,
        projection_names: ["run_summary"],
        emit_entity_events: false,
        emit_parity_failures: false
      )

    assert summary.sampled == 1
    assert summary.mismatches == 1
    assert summary.hash_mismatches == 1
    assert summary.lag_drift_nonzero == 1
    assert summary.max_abs_lag_drift == 1
    assert_in_delta summary.mismatch_rate, 1.0, 1.0e-9
    assert summary.status == :mismatch
  end

  @tag :sync_parity
  test "run_once reports missing khala document" do
    reset_parity_tables!()

    worker_id = unique_id("worker_parity_missing")
    document_id = "runtime/codex_worker_summary:#{worker_id}"
    payload = worker_payload(worker_id, document_id, 4)

    insert_checkpoint("codex_worker_summary", worker_id, document_id, 4, payload)

    summary =
      ParityAuditor.run_once(
        enabled: true,
        sample_size: 10,
        projection_names: ["codex_worker_summary"],
        emit_entity_events: false,
        emit_parity_failures: false
      )

    assert summary.sampled == 1
    assert summary.missing_documents == 1
    assert summary.mismatches == 1
    assert summary.hash_mismatches == 0
    assert summary.lag_drift_nonzero == 1
    assert summary.max_abs_lag_drift == 4
    assert summary.status == :mismatch
  end

  defp insert_checkpoint(projection_name, entity_id, document_id, seq, payload) do
    Repo.insert!(%ProjectionCheckpoint{
      projection_name: projection_name,
      entity_id: entity_id,
      document_id: document_id,
      last_runtime_seq: seq,
      projection_version: "khala_summary_v1",
      summary_hash: Projector.summary_hash_for_parity(payload),
      last_projected_at: @projected_at
    })
  end

  defp insert_run_summary(document_id, seq, payload) do
    Repo.insert!(%RunSummary{
      doc_key: document_id,
      doc_version: seq,
      payload: payload,
      payload_hash: payload_hash_bytes(payload)
    })
  end

  defp payload_hash_bytes(payload) do
    payload
    |> PayloadHash.canonical_json()
    |> then(&:crypto.hash(:sha256, &1))
  end

  defp run_payload(run_id, document_id, seq) do
    %{
      "document_id" => document_id,
      "kind" => "run_summary",
      "run_id" => run_id,
      "status" => "running",
      "runtime_source" => %{
        "entity" => "run",
        "run_id" => run_id,
        "seq" => seq
      },
      "projection_version" => "khala_summary_v1",
      "projected_at" => DateTime.to_iso8601(@projected_at)
    }
  end

  defp worker_payload(worker_id, document_id, seq) do
    %{
      "document_id" => document_id,
      "kind" => "codex_worker_summary",
      "worker_id" => worker_id,
      "status" => "running",
      "runtime_source" => %{
        "entity" => "codex_worker",
        "worker_id" => worker_id,
        "seq" => seq
      },
      "projection_version" => "khala_summary_v1",
      "projected_at" => DateTime.to_iso8601(@projected_at)
    }
  end

  defp unique_id(prefix), do: "#{prefix}_#{System.unique_integer([:positive])}"

  defp reset_parity_tables! do
    Repo.delete_all(ProjectionCheckpoint)
    Repo.delete_all(RunSummary)
    Repo.delete_all(CodexWorkerSummary)
  end
end
