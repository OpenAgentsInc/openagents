defmodule OpenAgentsRuntimeWeb.Plugs.InternalAuthTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: false

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunOwnership

  setup do
    case :ets.whereis(:openagents_runtime_auth_nonce_cache) do
      :undefined -> :ok
      table -> :ets.delete_all_objects(table)
    end

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

  test "rejects internal request without signature token", %{conn: conn} do
    conn = get(conn, ~p"/internal/v1/health")

    assert %{"error" => %{"code" => "unauthorized"}} = json_response(conn, 401)
  end

  test "rejects invalid signature token", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth(token: "v1.invalid.invalid")
      |> get(~p"/internal/v1/health")

    assert %{"error" => %{"code" => "unauthorized"}} = json_response(conn, 401)
  end

  test "rejects replayed nonce", %{conn: conn} do
    token = valid_signature_token()

    conn_1 = conn |> put_internal_auth(token: token) |> get(~p"/internal/v1/health")
    assert json_response(conn_1, 200)

    conn_2 = build_conn() |> put_internal_auth(token: token) |> get(~p"/internal/v1/health")
    assert %{"error" => %{"code" => "unauthorized"}} = json_response(conn_2, 401)
  end

  test "rejects claim mismatch for run context", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth(run_id: "run_other", thread_id: "thread_snapshot", user_id: 77)
      |> get(~p"/internal/v1/runs/run_snapshot/snapshot?thread_id=thread_snapshot")

    assert %{"error" => %{"code" => "forbidden"}} = json_response(conn, 403)
  end

  test "rejects claim mismatch when run_id is supplied in request body", %{conn: conn} do
    conn =
      conn
      |> put_internal_auth(run_id: "run_other", user_id: 77)
      |> post(~p"/internal/v1/tools/execute", %{
        "tool_pack" => "coding.v1",
        "manifest" => %{"manifest_version" => "coding.integration.v1"},
        "request" => %{},
        "run_id" => "run_snapshot"
      })

    assert %{"error" => %{"code" => "forbidden"}} = json_response(conn, 403)
  end

  test "allows valid signed internal request", %{conn: conn} do
    conn = conn |> put_internal_auth() |> get(~p"/internal/v1/health")

    assert %{"status" => "ok"} = json_response(conn, 200)
  end
end
