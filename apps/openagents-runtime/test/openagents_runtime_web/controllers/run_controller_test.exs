defmodule OpenAgentsRuntimeWeb.RunControllerTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: true

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
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
end
