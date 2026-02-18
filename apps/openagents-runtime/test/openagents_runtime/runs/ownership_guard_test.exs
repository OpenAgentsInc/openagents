defmodule OpenAgentsRuntime.Runs.OwnershipGuardTest do
  use OpenAgentsRuntime.DataCase, async: true

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.OwnershipGuard
  alias OpenAgentsRuntime.Runs.RunOwnership

  setup do
    Repo.insert!(%RunOwnership{
      run_id: "run_1",
      thread_id: "thread_1",
      user_id: 123,
      guest_scope: "guest_abc"
    })

    :ok
  end

  test "authorizes matching user ownership" do
    assert :ok = OwnershipGuard.authorize("run_1", "thread_1", %{user_id: 123})
  end

  test "authorizes matching guest ownership" do
    assert :ok = OwnershipGuard.authorize("run_1", "thread_1", %{guest_scope: "guest_abc"})
  end

  test "rejects mismatched principal" do
    assert {:error, :forbidden} = OwnershipGuard.authorize("run_1", "thread_1", %{user_id: 999})
  end

  test "rejects unknown run/thread mapping" do
    assert {:error, :not_found} = OwnershipGuard.authorize("run_2", "thread_2", %{user_id: 123})
  end

  test "rejects invalid principal" do
    assert {:error, :invalid_principal} = OwnershipGuard.authorize("run_1", "thread_1", %{})
  end
end
