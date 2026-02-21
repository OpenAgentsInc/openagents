defmodule OpenAgentsRuntime.ReleaseTest do
  use OpenAgentsRuntime.DataCase, async: false

  test "migrate_and_verify! succeeds when required runtime tables exist" do
    assert :ok = OpenAgentsRuntime.Release.migrate_and_verify!()
  end

  test "verify_required_tables! raises when a required table is missing" do
    assert_raise RuntimeError, ~r/required runtime table missing after migrations/, fn ->
      OpenAgentsRuntime.Release.verify_required_tables!([
        "runtime.__missing_release_verify_table"
      ])
    end
  end
end
