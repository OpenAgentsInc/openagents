defmodule OpenAgentsRuntime.Runs.LeasesTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Leases
  alias OpenAgentsRuntime.Runs.Run

  setup do
    Repo.insert!(%Run{
      run_id: "run_lease_1",
      thread_id: "thread_lease_1",
      status: "running",
      owner_user_id: 55,
      latest_seq: 0
    })

    :ok
  end

  test "acquires a fresh lease" do
    now = DateTime.utc_now()

    assert {:ok, lease} =
             Leases.acquire("run_lease_1", "worker-a", now: now, ttl_seconds: 20)

    assert lease.lease_owner == "worker-a"
    assert DateTime.compare(lease.lease_expires_at, now) == :gt
  end

  test "returns lease_held while unexpired lease belongs to another owner" do
    now = DateTime.utc_now()

    assert {:ok, _lease} = Leases.acquire("run_lease_1", "worker-a", now: now, ttl_seconds: 20)

    assert {:error, :lease_held} =
             Leases.acquire("run_lease_1", "worker-b", now: now, ttl_seconds: 20)
  end

  test "renews existing lease when same owner reacquires" do
    now = DateTime.utc_now()

    assert {:ok, lease_1} = Leases.acquire("run_lease_1", "worker-a", now: now, ttl_seconds: 5)

    assert {:ok, lease_2} =
             Leases.acquire("run_lease_1", "worker-a",
               now: DateTime.add(now, 2, :second),
               ttl_seconds: 30
             )

    assert DateTime.compare(lease_2.lease_expires_at, lease_1.lease_expires_at) == :gt
  end

  test "safe steal requires expired lease and no progress movement" do
    now = DateTime.utc_now()

    assert {:ok, _lease} = Leases.acquire("run_lease_1", "worker-a", now: now, ttl_seconds: 1)
    assert {:ok, _lease} = Leases.mark_progress("run_lease_1", "worker-a", 5)

    expired_now = DateTime.add(now, 3, :second)

    assert {:error, :lease_progressed} =
             Leases.acquire("run_lease_1", "worker-b",
               now: expired_now,
               ttl_seconds: 30,
               observed_progress_seq: 0
             )

    assert {:ok, stolen} =
             Leases.acquire("run_lease_1", "worker-b",
               now: expired_now,
               ttl_seconds: 30,
               observed_progress_seq: 5
             )

    assert stolen.lease_owner == "worker-b"
  end

  test "renew and mark_progress enforce owner" do
    assert {:ok, _lease} = Leases.acquire("run_lease_1", "worker-a")

    assert {:error, :not_owner} = Leases.renew("run_lease_1", "worker-b")
    assert {:error, :not_owner} = Leases.mark_progress("run_lease_1", "worker-b", 1)
  end

  test "emits lease telemetry with low-cardinality action/result tags" do
    handler_id = "lease-telemetry-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        handler_id,
        [:openagents_runtime, :lease, :operation],
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:lease_event, measurements, metadata})
        end,
        self()
      )

    on_exit(fn -> :telemetry.detach(handler_id) end)

    now = DateTime.utc_now()

    assert {:ok, _lease} =
             Leases.acquire("run_lease_1", "worker-a", now: now, ttl_seconds: 20)

    assert_receive {:lease_event, measurements, metadata}, 1_000
    assert measurements.count == 1
    assert metadata.action == "acquire"
    assert metadata.result == "acquired"
    assert metadata.run_id == "run_lease_1"
    assert metadata.lease_owner == "worker-a"
  end
end
