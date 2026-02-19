defmodule OpenAgentsRuntimeWeb.CodexWorkerControllerTest do
  use OpenAgentsRuntimeWeb.ConnCase, async: false

  test "create snapshot request and stop worker", %{conn: conn} do
    create_conn =
      conn
      |> put_internal_auth(user_id: 900)
      |> post(~p"/internal/v1/codex/workers", %{
        "worker_id" => unique_id("codexw"),
        "workspace_ref" => "workspace://demo"
      })

    assert %{
             "data" => %{
               "workerId" => worker_id,
               "status" => "running",
               "idempotentReplay" => false
             }
           } = json_response(create_conn, 202)

    snapshot_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 900)
      |> get(~p"/internal/v1/codex/workers/#{worker_id}/snapshot")

    assert %{"data" => %{"worker_id" => ^worker_id, "status" => "running"}} =
             json_response(snapshot_conn, 200)

    request_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 900)
      |> post(~p"/internal/v1/codex/workers/#{worker_id}/requests", %{
        "request" => %{
          "request_id" => "req_#{System.unique_integer([:positive])}",
          "method" => "thread/start",
          "params" => %{"prompt" => "hello"}
        }
      })

    assert %{"data" => %{"worker_id" => ^worker_id, "ok" => true}} =
             json_response(request_conn, 200)

    stop_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 900)
      |> post(~p"/internal/v1/codex/workers/#{worker_id}/stop", %{"reason" => "done"})

    assert %{
             "data" => %{
               "worker_id" => ^worker_id,
               "status" => "stopped",
               "idempotent_replay" => false
             }
           } =
             json_response(stop_conn, 202)

    replay_stop_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 900)
      |> post(~p"/internal/v1/codex/workers/#{worker_id}/stop", %{"reason" => "done"})

    assert %{"data" => %{"idempotent_replay" => true}} = json_response(replay_stop_conn, 200)
  end

  test "stream validates cursor input", %{conn: conn} do
    worker_id = unique_id("codexw")

    conn
    |> put_internal_auth(user_id: 901)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => worker_id})
    |> json_response(202)

    invalid_cursor_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 901)
      |> get(~p"/internal/v1/codex/workers/#{worker_id}/stream", %{"cursor" => "abc"})

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(invalid_cursor_conn, 400)
  end

  test "worker ownership is enforced", %{conn: conn} do
    worker_id = unique_id("codexw")

    conn
    |> put_internal_auth(user_id: 902)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => worker_id})
    |> json_response(202)

    forbidden_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 903)
      |> get(~p"/internal/v1/codex/workers/#{worker_id}/snapshot")

    assert %{"error" => %{"code" => "forbidden"}} = json_response(forbidden_conn, 403)
  end

  test "list returns principal-owned workers and projection checkpoint status", %{conn: conn} do
    owner_id = 904
    other_id = 905

    owner_worker_running = unique_id("codexw")
    owner_worker_stopped = unique_id("codexw")
    other_worker = unique_id("codexw")

    conn
    |> put_internal_auth(user_id: owner_id)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => owner_worker_running})
    |> json_response(202)

    conn
    |> recycle()
    |> put_internal_auth(user_id: owner_id)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => owner_worker_stopped})
    |> json_response(202)

    conn
    |> recycle()
    |> put_internal_auth(user_id: owner_id)
    |> post(~p"/internal/v1/codex/workers/#{owner_worker_stopped}/stop", %{"reason" => "done"})
    |> json_response(202)

    conn
    |> recycle()
    |> put_internal_auth(user_id: other_id)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => other_worker})
    |> json_response(202)

    response =
      conn
      |> recycle()
      |> put_internal_auth(user_id: owner_id)
      |> get(~p"/internal/v1/codex/workers?status=running&limit=10")
      |> json_response(200)

    assert %{"data" => workers} = response
    assert length(workers) == 1

    assert [
             %{
               "worker_id" => ^owner_worker_running,
               "status" => "running",
               "convex_projection" => %{
                 "document_id" => _document_id,
                 "status" => convex_status
               }
             }
           ] = workers

    assert convex_status in ["in_sync", "lagging"]
  end

  test "list validates query parameters", %{conn: conn} do
    invalid_limit_conn =
      conn
      |> put_internal_auth(user_id: 906)
      |> get(~p"/internal/v1/codex/workers?limit=0")

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(invalid_limit_conn, 400)

    invalid_status_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 906)
      |> get(~p"/internal/v1/codex/workers?status=unknown")

    assert %{"error" => %{"code" => "invalid_request"}} = json_response(invalid_status_conn, 400)
  end

  defp unique_id(prefix), do: "#{prefix}_#{System.unique_integer([:positive])}"
end
