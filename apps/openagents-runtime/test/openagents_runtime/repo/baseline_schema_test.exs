defmodule OpenAgentsRuntime.Repo.BaselineSchemaTest do
  use OpenAgentsRuntime.DataCase, async: true

  alias OpenAgentsRuntime.Repo

  test "runtime baseline schema objects exist" do
    assert {:ok, %{rows: [["runtime.runs"]]}} =
             Repo.query("SELECT to_regclass('runtime.runs')::text")

    assert {:ok, %{rows: [["runtime.global_event_id_seq"]]}} =
             Repo.query("SELECT to_regclass('runtime.global_event_id_seq')::text")

    assert {:ok, %{rows: [["runtime.tool_tasks"]]}} =
             Repo.query("SELECT to_regclass('runtime.tool_tasks')::text")

    assert {:ok, %{rows: [["runtime.timeline_events"]]}} =
             Repo.query("SELECT to_regclass('runtime.timeline_events')::text")

    assert {:ok, %{rows: [["runtime.memory_chunks"]]}} =
             Repo.query("SELECT to_regclass('runtime.memory_chunks')::text")

    assert {:ok, %{rows: [["runtime.memory_retention_policies"]]}} =
             Repo.query("SELECT to_regclass('runtime.memory_retention_policies')::text")

    assert {:ok, %{rows: [["runtime.memory_compactions"]]}} =
             Repo.query("SELECT to_regclass('runtime.memory_compactions')::text")

    assert {:ok, %{rows: [["runtime.memory_rollups"]]}} =
             Repo.query("SELECT to_regclass('runtime.memory_rollups')::text")

    assert {:ok, %{rows: [["runtime.ds_artifact_pointers"]]}} =
             Repo.query("SELECT to_regclass('runtime.ds_artifact_pointers')::text")

    assert {:ok, %{rows: [["runtime.ds_compile_reports"]]}} =
             Repo.query("SELECT to_regclass('runtime.ds_compile_reports')::text")

    assert {:ok, %{rows: [["runtime.ds_eval_reports"]]}} =
             Repo.query("SELECT to_regclass('runtime.ds_eval_reports')::text")

    assert {:ok, %{rows: [["runtime.ds_pointer_audits"]]}} =
             Repo.query("SELECT to_regclass('runtime.ds_pointer_audits')::text")

    assert {:ok, %{rows: [["runtime.sync_stream_events"]]}} =
             Repo.query("SELECT to_regclass('runtime.sync_stream_events')::text")

    assert {:ok, %{rows: [["runtime.sync_run_summaries"]]}} =
             Repo.query("SELECT to_regclass('runtime.sync_run_summaries')::text")

    assert {:ok, %{rows: [["runtime.sync_codex_worker_summaries"]]}} =
             Repo.query("SELECT to_regclass('runtime.sync_codex_worker_summaries')::text")
  end
end
