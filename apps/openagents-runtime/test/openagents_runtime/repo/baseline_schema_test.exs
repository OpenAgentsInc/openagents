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
  end
end
