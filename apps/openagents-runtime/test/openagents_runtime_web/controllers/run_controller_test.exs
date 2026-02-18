defmodule OpenAgentsRuntimeWeb.RunControllerTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: true

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.RunOwnership

  setup do
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
end
