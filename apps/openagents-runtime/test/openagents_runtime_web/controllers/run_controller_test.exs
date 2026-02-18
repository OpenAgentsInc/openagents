defmodule OpenAgentsRuntimeWeb.RunControllerTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: false

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Runs.RunOwnership

  setup do
    Repo.insert!(%Run{
      run_id: "run_snapshot",
      thread_id: "thread_snapshot",
      status: "running",
      owner_user_id: 77,
      latest_seq: 0
    })

    Repo.insert!(%RunOwnership{run_id: "run_snapshot", thread_id: "thread_snapshot", user_id: 77})
    :ok
  end

  test "returns snapshot for authorized user", %{conn: conn} do
    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> get(~p"/internal/v1/runs/run_snapshot/snapshot?thread_id=thread_snapshot")

    assert %{
             "runId" => "run_snapshot",
             "threadId" => "thread_snapshot",
             "status" => "unknown",
             "latestSeq" => 0,
             "updatedAt" => _
           } = json_response(conn, 200)
  end

  test "returns forbidden for cross-tenant request", %{conn: conn} do
    conn =
      conn
      |> put_req_header("x-oa-user-id", "999")
      |> get(~p"/internal/v1/runs/run_snapshot/snapshot?thread_id=thread_snapshot")

    assert %{"error" => %{"code" => "forbidden"}} = json_response(conn, 403)
  end

  test "returns unauthorized for missing principal headers", %{conn: conn} do
    conn = get(conn, ~p"/internal/v1/runs/run_snapshot/snapshot?thread_id=thread_snapshot")

    assert %{"error" => %{"code" => "unauthorized"}} = json_response(conn, 401)
  end

  test "returns not_found when run/thread is missing", %{conn: conn} do
    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> get(~p"/internal/v1/runs/run_missing/snapshot?thread_id=thread_missing")

    assert %{"error" => %{"code" => "not_found"}} = json_response(conn, 404)
  end

  test "returns invalid request when thread_id is missing", %{conn: conn} do
    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> get(~p"/internal/v1/runs/run_snapshot/snapshot")

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(conn, 400)
  end

  test "append_frame accepts first write and marks duplicate as idempotent replay", %{conn: conn} do
    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> post(~p"/internal/v1/runs/run_snapshot/frames", %{
        "thread_id" => "thread_snapshot",
        "frame_id" => "frame_1",
        "type" => "user_message",
        "payload" => %{"text" => "hello"}
      })

    assert %{
             "runId" => "run_snapshot",
             "frameId" => "frame_1",
             "status" => "accepted",
             "idempotentReplay" => false
           } = json_response(conn, 202)

    conn =
      build_conn()
      |> put_req_header("x-oa-user-id", "77")
      |> put_req_header("content-type", "application/json")
      |> post(~p"/internal/v1/runs/run_snapshot/frames", %{
        "thread_id" => "thread_snapshot",
        "frame_id" => "frame_1",
        "type" => "user_message",
        "payload" => %{"text" => "hello"}
      })

    assert %{
             "runId" => "run_snapshot",
             "frameId" => "frame_1",
             "status" => "accepted",
             "idempotentReplay" => true
           } = json_response(conn, 200)
  end

  test "append_frame returns conflict when duplicate frame_id payload differs", %{conn: conn} do
    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> post(~p"/internal/v1/runs/run_snapshot/frames", %{
        "thread_id" => "thread_snapshot",
        "frame_id" => "frame_conflict",
        "type" => "user_message",
        "payload" => %{"text" => "v1"}
      })

    assert json_response(conn, 202)

    conn =
      build_conn()
      |> put_req_header("x-oa-user-id", "77")
      |> put_req_header("content-type", "application/json")
      |> post(~p"/internal/v1/runs/run_snapshot/frames", %{
        "thread_id" => "thread_snapshot",
        "frame_id" => "frame_conflict",
        "type" => "user_message",
        "payload" => %{"text" => "v2"}
      })

    assert %{"error" => %{"code" => "conflict"}} = json_response(conn, 409)
  end

  test "stream returns SSE events after cursor", %{conn: conn} do
    assert {:ok, _} = RunEvents.append_event("run_snapshot", "run.delta", %{"delta" => "one"})
    assert {:ok, _} = RunEvents.append_event("run_snapshot", "run.delta", %{"delta" => "two"})

    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> get(~p"/internal/v1/runs/run_snapshot/stream?thread_id=thread_snapshot&cursor=1")

    assert conn.status == 200
    assert List.first(get_resp_header(conn, "content-type")) =~ "text/event-stream"
    assert conn.resp_body =~ "id: 2"
    assert conn.resp_body =~ "\"type\":\"run.delta\""
  end

  test "stream resumes from Last-Event-ID header", %{conn: conn} do
    assert {:ok, _} = RunEvents.append_event("run_snapshot", "run.delta", %{"delta" => "one"})
    assert {:ok, _} = RunEvents.append_event("run_snapshot", "run.delta", %{"delta" => "two"})

    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> put_req_header("last-event-id", "1")
      |> get(~p"/internal/v1/runs/run_snapshot/stream?thread_id=thread_snapshot")

    assert conn.status == 200
    assert conn.resp_body =~ "id: 2"
  end

  test "stream returns invalid_request when cursor query and Last-Event-ID mismatch", %{
    conn: conn
  } do
    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> put_req_header("last-event-id", "2")
      |> get(~p"/internal/v1/runs/run_snapshot/stream?thread_id=thread_snapshot&cursor=1")

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(conn, 400)
  end

  test "stream returns stale_cursor when cursor is below retention floor", %{conn: conn} do
    assert {:ok, _} = RunEvents.append_event("run_snapshot", "run.delta", %{"delta" => "one"})
    assert {:ok, _} = RunEvents.append_event("run_snapshot", "run.delta", %{"delta" => "two"})
    assert {:ok, _} = RunEvents.append_event("run_snapshot", "run.delta", %{"delta" => "three"})

    delete_query =
      from(event in RunEvent,
        where: event.run_id == "run_snapshot" and event.seq < 3
      )

    assert {2, _} = Repo.delete_all(delete_query)

    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> get(~p"/internal/v1/runs/run_snapshot/stream?thread_id=thread_snapshot&cursor=0")

    assert %{"error" => %{"code" => "stale_cursor"}} = json_response(conn, 410)
  end

  test "stream wakes up and emits events appended during tail window" do
    parent = self()

    stream_pid =
      spawn(fn ->
        receive do
          {:go, test_pid} ->
            conn =
              build_conn()
              |> put_req_header("x-oa-user-id", "77")
              |> get(
                ~p"/internal/v1/runs/run_snapshot/stream?thread_id=thread_snapshot&cursor=0&tail_ms=500"
              )

            send(test_pid, {:stream_response, conn.status, conn.resp_body})
        end
      end)

    Ecto.Adapters.SQL.Sandbox.allow(Repo, self(), stream_pid)
    send(stream_pid, {:go, parent})
    Process.sleep(75)

    assert {:ok, _event} =
             RunEvents.append_event("run_snapshot", "run.delta", %{"delta" => "late"})

    assert_receive {:stream_response, 200, body}, 2_000
    assert body =~ "id: 1"
    assert body =~ "\"delta\":\"late\""
  end

  test "stream rejects invalid tail timeout", %{conn: conn} do
    conn =
      conn
      |> put_req_header("x-oa-user-id", "77")
      |> get(~p"/internal/v1/runs/run_snapshot/stream?thread_id=thread_snapshot&tail_ms=abc")

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(conn, 400)
  end
end
