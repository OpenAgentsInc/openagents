defmodule OpenAgentsRuntime.Runs.EventNotifierTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias Ecto.Adapters.SQL.Sandbox
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.EventListener
  alias OpenAgentsRuntime.Runs.EventNotifier
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvents

  test "append_event emits pg_notify payload after commit" do
    run_id = "run_notify_#{System.unique_integer([:positive])}"

    Sandbox.unboxed_run(Repo, fn ->
      Repo.insert!(%Run{
        run_id: run_id,
        thread_id: "thread_notify_1",
        status: "running",
        owner_user_id: 99,
        latest_seq: 0
      })

      notifications_config =
        Repo.config()
        |> Keyword.take([:hostname, :port, :username, :password, :database, :ssl, :socket_dir])

      {:ok, notifications_pid} = Postgrex.Notifications.start_link(notifications_config)

      {:ok, _listen_ref} =
        Postgrex.Notifications.listen(notifications_pid, EventNotifier.channel())

      assert {:ok, event} = RunEvents.append_event(run_id, "run.started", %{})

      assert_receive {:notification, ^notifications_pid, _ref, "runtime_run_events", payload},
                     1_000

      assert {:ok, %{run_id: decoded_run_id, seq: seq}} = EventNotifier.decode_payload(payload)
      assert decoded_run_id == run_id
      assert seq == event.seq
    end)
  end

  test "listener broadcasts wakeup events to run topic" do
    run_id = "run_notify_#{System.unique_integer([:positive])}"

    Sandbox.unboxed_run(Repo, fn ->
      Repo.insert!(%Run{
        run_id: run_id,
        thread_id: "thread_notify_1",
        status: "running",
        owner_user_id: 99,
        latest_seq: 0
      })

      assert :ok = EventListener.subscribe(run_id)

      assert {:ok, _event} = RunEvents.append_event(run_id, "run.delta", %{"x" => 1})

      assert_receive {:run_event_notification, %{run_id: ^run_id, seq: 1}}, 1_000
    end)
  end

  test "failed append does not emit pg_notify" do
    Sandbox.unboxed_run(Repo, fn ->
      notifications_config =
        Repo.config()
        |> Keyword.take([:hostname, :port, :username, :password, :database, :ssl, :socket_dir])

      {:ok, notifications_pid} = Postgrex.Notifications.start_link(notifications_config)

      {:ok, _listen_ref} =
        Postgrex.Notifications.listen(notifications_pid, EventNotifier.channel())

      assert {:error, :run_not_found} = RunEvents.append_event("missing_run", "run.started", %{})

      refute_receive {:notification, ^notifications_pid, _ref, "runtime_run_events", _payload},
                     300
    end)
  end
end
