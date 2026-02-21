defmodule OpenAgentsRuntime.Load.RuntimeShapeLoadTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: false

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Executor
  alias OpenAgentsRuntime.Runs.Frames
  alias OpenAgentsRuntime.Runs.Janitor
  alias OpenAgentsRuntime.Runs.Leases
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Runs.RunOwnership

  @tag :load
  test "concurrent SSE consumers stay consistent under delayed event production" do
    run_id = unique_run_id("load_sse")
    thread_id = "thread_#{run_id}"
    user_id = 8101
    insert_run_with_ownership(run_id, thread_id, user_id, "running")
    parent = self()

    for idx <- 1..8 do
      spawn(fn ->
        Ecto.Adapters.SQL.Sandbox.allow(Repo, parent, self())
        Process.sleep(idx * 5)

        conn =
          build_conn()
          |> put_internal_auth(run_id: run_id, thread_id: thread_id, user_id: user_id)
          |> get(
            ~p"/internal/v1/runs/#{run_id}/stream?thread_id=#{thread_id}&cursor=0&tail_ms=400"
          )

        send(parent, {:stream_result, conn.status, conn.resp_body})
      end)
    end

    Process.sleep(75)

    for idx <- 1..5 do
      assert {:ok, _event} =
               RunEvents.append_event(run_id, "run.delta", %{"delta" => "chunk-#{idx}"})

      Process.sleep(20)
    end

    assert {:ok, _event} =
             RunEvents.append_event(run_id, "run.finished", %{
               "status" => "succeeded",
               "reason_class" => "completed",
               "reason" => "load_test_done"
             })

    for _ <- 1..8 do
      assert_receive {:stream_result, 200, body}, 2_000
      assert body =~ "[DONE]"

      ids = parse_unique_sse_ids(body)
      assert ids == Enum.to_list(1..6)
    end
  end

  @tag :load
  test "burst frame ingestion preserves contiguous event ordering" do
    run_id = unique_run_id("load_burst")
    thread_id = "thread_#{run_id}"
    user_id = 8102
    insert_run_with_ownership(run_id, thread_id, user_id, "created")
    parent = self()

    tasks =
      for idx <- 1..30 do
        Task.async(fn ->
          Ecto.Adapters.SQL.Sandbox.allow(Repo, parent, self())

          conn =
            build_conn()
            |> put_internal_auth(run_id: run_id, thread_id: thread_id, user_id: user_id)
            |> put_req_header("content-type", "application/json")
            |> post(~p"/internal/v1/runs/#{run_id}/frames", %{
              "thread_id" => thread_id,
              "frame_id" => "frame_burst_#{idx}",
              "type" => "user_message",
              "payload" => %{"text" => "burst-#{idx}"}
            })

          conn.status
        end)
      end

    statuses = Enum.map(tasks, &Task.await(&1, 3_000))
    assert Enum.all?(statuses, &(&1 in [200, 202]))

    finish_conn =
      build_conn()
      |> put_internal_auth(run_id: run_id, thread_id: thread_id, user_id: user_id)
      |> post(~p"/internal/v1/runs/#{run_id}/frames", %{
        "thread_id" => thread_id,
        "frame_id" => "frame_burst_finish",
        "type" => "complete",
        "payload" => %{}
      })

    assert finish_conn.status in [200, 202]

    assert_eventually(fn ->
      Repo.get!(Run, run_id).status == "succeeded"
    end)

    seqs =
      RunEvents.list_after(run_id, 0)
      |> Enum.map(& &1.seq)

    assert seqs == Enum.to_list(1..length(seqs))
    assert length(seqs) >= 3
  end

  @tag :load
  test "cancel storms remain idempotent with a single canceled terminal event per run" do
    runs =
      for idx <- 1..10 do
        run_id = unique_run_id("load_cancel_#{idx}")
        thread_id = "thread_#{run_id}"
        user_id = 8200 + idx
        insert_run_with_ownership(run_id, thread_id, user_id, "running")
        %{run_id: run_id, thread_id: thread_id, user_id: user_id}
      end

    parent = self()

    cancel_tasks =
      for run <- runs, attempt <- 1..3 do
        Task.async(fn ->
          Ecto.Adapters.SQL.Sandbox.allow(Repo, parent, self())

          conn =
            build_conn()
            |> put_internal_auth(
              run_id: run.run_id,
              thread_id: run.thread_id,
              user_id: run.user_id
            )
            |> post(~p"/internal/v1/runs/#{run.run_id}/cancel", %{
              "thread_id" => run.thread_id,
              "reason" => "storm-#{attempt}"
            })

          conn.status
        end)
      end

    statuses = Enum.map(cancel_tasks, &Task.await(&1, 3_000))
    assert Enum.all?(statuses, &(&1 in [200, 202]))

    for run <- runs do
      assert_eventually(fn ->
        Repo.get!(Run, run.run_id).status == "canceled"
      end)

      canceled_finishes =
        from(event in RunEvent,
          where:
            event.run_id == ^run.run_id and event.event_type == "run.finished" and
              fragment("?->>'status' = 'canceled'", event.payload),
          select: count()
        )
        |> Repo.one()

      assert canceled_finishes == 1
    end
  end

  @tag :load
  @tag :chaos_drill
  test "janitor recovery after executor loss supports cursor resume without gaps" do
    run_id = unique_run_id("load_recover")
    thread_id = "thread_#{run_id}"
    user_id = 8301
    insert_run_with_ownership(run_id, thread_id, user_id, "created")

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "recover_1",
               type: "user_message",
               payload: %{"text" => "recover me"}
             })

    assert {:ok, _} =
             Frames.append_frame(run_id, %{
               frame_id: "recover_2",
               type: "complete",
               payload: %{}
             })

    now = DateTime.utc_now()
    assert {:ok, _lease} = Leases.acquire(run_id, "worker-lost", now: now, ttl_seconds: 1)

    summary =
      Janitor.run_once(
        now: DateTime.add(now, 5, :second),
        max_recovery_attempts: 3,
        recovery_cooldown_ms: 0
      )

    assert summary.scanned == 1
    assert summary.resumed == 1

    assert {:ok, _result} =
             Executor.run_once(run_id,
               lease_owner: "load-recover-worker",
               now: DateTime.add(now, 6, :second)
             )

    assert Repo.get!(Run, run_id).status == "succeeded"

    full_stream_conn =
      build_conn()
      |> put_internal_auth(run_id: run_id, thread_id: thread_id, user_id: user_id)
      |> get(~p"/internal/v1/runs/#{run_id}/stream?thread_id=#{thread_id}&cursor=0&tail_ms=75")

    assert full_stream_conn.status == 200
    full_ids = parse_unique_sse_ids(full_stream_conn.resp_body)
    assert full_ids == Enum.to_list(1..List.last(full_ids))

    resume_cursor = Enum.at(full_ids, 1)

    resumed_stream_conn =
      build_conn()
      |> put_internal_auth(run_id: run_id, thread_id: thread_id, user_id: user_id)
      |> get(
        ~p"/internal/v1/runs/#{run_id}/stream?thread_id=#{thread_id}&cursor=#{resume_cursor}&tail_ms=75"
      )

    assert resumed_stream_conn.status == 200
    resumed_ids = parse_unique_sse_ids(resumed_stream_conn.resp_body)
    assert resumed_ids == Enum.filter(full_ids, &(&1 > resume_cursor))
  end

  defp insert_run_with_ownership(run_id, thread_id, user_id, status) do
    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: thread_id,
      status: status,
      owner_user_id: user_id,
      latest_seq: 0
    })

    Repo.insert!(%RunOwnership{
      run_id: run_id,
      thread_id: thread_id,
      user_id: user_id
    })
  end

  defp parse_unique_sse_ids(body) do
    body
    |> String.split("\n")
    |> Enum.flat_map(fn line ->
      case String.trim(line) do
        <<"id: ", id::binary>> ->
          case Integer.parse(String.trim(id)) do
            {value, ""} -> [value]
            _ -> []
          end

        _ ->
          []
      end
    end)
    |> Enum.uniq()
    |> Enum.sort()
  end

  defp unique_run_id(prefix) do
    "#{prefix}_#{System.unique_integer([:positive])}"
  end

  defp assert_eventually(fun, attempts \\ 80, sleep_ms \\ 25)

  defp assert_eventually(fun, attempts, _sleep_ms) when attempts <= 0 do
    assert fun.()
  end

  defp assert_eventually(fun, attempts, sleep_ms) do
    if fun.() do
      :ok
    else
      Process.sleep(sleep_ms)
      assert_eventually(fun, attempts - 1, sleep_ms)
    end
  end
end
