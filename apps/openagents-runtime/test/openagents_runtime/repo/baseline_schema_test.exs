defmodule OpenAgentsRuntime.Repo.BaselineSchemaTest do
  use OpenAgentsRuntime.DataCase, async: true

  alias OpenAgentsRuntime.Repo

  test "runtime baseline schema objects exist" do
    assert {:ok, %{rows: [["runtime.runs"]]}} =
             Repo.query("SELECT to_regclass('runtime.runs')::text")

    assert {:ok, %{rows: [["runtime.global_event_id_seq"]]}} =
             Repo.query("SELECT to_regclass('runtime.global_event_id_seq')::text")
  end
end
