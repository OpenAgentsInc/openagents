defmodule OpenAgentsRuntime.Spend.ReservationsTest do
  use OpenAgentsRuntime.DataCase, async: false

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunOwnership
  alias OpenAgentsRuntime.Spend.Authorizations
  alias OpenAgentsRuntime.Spend.Reservations
  alias OpenAgentsRuntime.Spend.SpendAuthorization
  alias OpenAgentsRuntime.Spend.SpendReservation

  setup do
    run_id = unique_id("run_reservation")
    thread_id = unique_id("thread_reservation")
    owner_user_id = System.unique_integer([:positive])

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: "running",
      owner_user_id: owner_user_id,
      latest_seq: 0
    })

    Repo.insert!(%RunOwnership{
      run_id: run_id,
      thread_id: thread_id,
      user_id: owner_user_id
    })

    authorization =
      insert_authorization!(%{
        owner_user_id: owner_user_id,
        run_id: run_id,
        max_total_sats: 100
      })

    {:ok, run_id: run_id, authorization: authorization}
  end

  test "reserve enforces budget atomically under race", ctx do
    auth_id = ctx.authorization.authorization_id
    run_id = ctx.run_id

    tasks = [
      Task.async(fn -> Reservations.reserve(auth_id, run_id, "tool_a", 70) end),
      Task.async(fn -> Reservations.reserve(auth_id, run_id, "tool_b", 40) end)
    ]

    results = Enum.map(tasks, &Task.await(&1, 5_000))
    success_count = Enum.count(results, &match?({:ok, _}, &1))
    over_budget_count = Enum.count(results, &match?({:error, :over_budget}, &1))

    assert success_count == 1
    assert over_budget_count == 1
  end

  test "duplicate reserve and commit are idempotent and do not double count", ctx do
    auth_id = ctx.authorization.authorization_id
    run_id = ctx.run_id

    assert {:ok, first_reserve} = Reservations.reserve(auth_id, run_id, "tool_charge", 35)
    refute first_reserve.idempotent_replay

    assert {:ok, replay_reserve} = Reservations.reserve(auth_id, run_id, "tool_charge", 35)
    assert replay_reserve.idempotent_replay
    assert replay_reserve.reservation.id == first_reserve.reservation.id

    assert {:ok, first_commit} =
             Reservations.commit(auth_id, run_id, "tool_charge",
               provider_correlation_id: "corr_1",
               provider_idempotency_key: "idem_1"
             )

    refute first_commit.idempotent_replay
    assert first_commit.reservation.state == "committed"

    assert {:ok, replay_commit} = Reservations.commit(auth_id, run_id, "tool_charge")
    assert replay_commit.idempotent_replay

    authorization = Repo.get!(SpendAuthorization, auth_id)
    assert authorization.spent_sats == 35
    assert authorization.reserved_sats == 0
  end

  test "recover_stuck marks stale reservations for reconciliation and reconcile release clears reserve",
       ctx do
    auth_id = ctx.authorization.authorization_id
    run_id = ctx.run_id

    assert {:ok, _reserve} = Reservations.reserve(auth_id, run_id, "tool_stuck", 20)

    stale_timestamp = DateTime.add(DateTime.utc_now(), -300, :second)

    from(reservation in SpendReservation,
      where:
        reservation.authorization_id == ^auth_id and reservation.run_id == ^run_id and
          reservation.tool_call_id == "tool_stuck"
    )
    |> Repo.update_all(set: [reserved_at: stale_timestamp])

    assert {:ok, [recovered]} =
             Reservations.recover_stuck(
               stale_before: DateTime.add(DateTime.utc_now(), -60, :second)
             )

    assert recovered.reservation.state == "reconcile_required"
    assert recovered.idempotent_replay == false

    assert {:ok, released} =
             Reservations.reconcile(auth_id, run_id, "tool_stuck", :release,
               failure_reason: "provider_timeout"
             )

    assert released.reservation.state == "released"
    assert %DateTime{} = released.reservation.reconciled_at
    assert released.reservation.failure_reason == "provider_timeout"

    authorization = Repo.get!(SpendAuthorization, auth_id)
    assert authorization.spent_sats == 0
    assert authorization.reserved_sats == 0
  end

  test "reserve rejects conflicting amount reuse for same reservation identity", ctx do
    auth_id = ctx.authorization.authorization_id
    run_id = ctx.run_id

    assert {:ok, _} = Reservations.reserve(auth_id, run_id, "tool_conflict", 25)

    assert {:error, :idempotency_conflict} =
             Reservations.reserve(auth_id, run_id, "tool_conflict", 30)
  end

  defp insert_authorization!(attrs) do
    payload =
      Map.merge(
        %{
          mode: "delegated_budget",
          spent_sats: 0,
          reserved_sats: 0,
          constraints: %{},
          metadata: %{}
        },
        attrs
      )

    case Authorizations.create(payload) do
      {:ok, authorization} -> authorization
      {:error, changeset} -> flunk("authorization insert failed: #{inspect(changeset.errors)}")
    end
  end

  defp unique_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
