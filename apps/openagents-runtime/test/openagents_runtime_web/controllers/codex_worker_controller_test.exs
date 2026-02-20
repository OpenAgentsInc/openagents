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

    assert %{
             "data" => %{
               "worker_id" => ^worker_id,
               "status" => "running",
               "heartbeat_state" => "fresh",
               "heartbeat_stale_after_ms" => stale_after_ms
             }
           } = json_response(snapshot_conn, 200)

    assert is_integer(stale_after_ms) and stale_after_ms > 0

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

    stopped_request_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 900)
      |> post(~p"/internal/v1/codex/workers/#{worker_id}/requests", %{
        "request" => %{"request_id" => unique_id("req"), "method" => "thread/start"}
      })

    assert %{"error" => %{"code" => "conflict"}} = json_response(stopped_request_conn, 409)

    resume_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 900)
      |> post(~p"/internal/v1/codex/workers", %{"worker_id" => worker_id})

    assert %{
             "data" => %{
               "workerId" => ^worker_id,
               "status" => "running",
               "idempotentReplay" => false
             }
           } = json_response(resume_conn, 202)
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

  test "ingests desktop event payload into durable worker stream", %{conn: conn} do
    worker_id = unique_id("codexw")

    conn
    |> put_internal_auth(user_id: 907)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => worker_id})
    |> json_response(202)

    ingest_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 907)
      |> post(~p"/internal/v1/codex/workers/#{worker_id}/events", %{
        "event" => %{
          "event_type" => "worker.event",
          "payload" => %{
            "source" => "desktop",
            "method" => "turn/started",
            "params" => %{"turnId" => "turn_1"}
          }
        }
      })

    assert %{
             "data" => %{
               "worker_id" => ^worker_id,
               "event_type" => "worker.event",
               "seq" => seq
             }
           } = json_response(ingest_conn, 202)

    assert is_integer(seq) and seq >= 2

    snapshot_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 907)
      |> get(~p"/internal/v1/codex/workers/#{worker_id}/snapshot")

    assert %{"data" => %{"latest_seq" => latest_seq}} = json_response(snapshot_conn, 200)
    assert latest_seq >= seq
  end

  test "ingests handshake envelopes and exposes them on worker stream", %{conn: conn} do
    worker_id = unique_id("codexw")
    handshake_id = "hs_#{System.unique_integer([:positive])}"

    conn
    |> put_internal_auth(user_id: 910)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => worker_id})
    |> json_response(202)

    ios_ingest_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 910)
      |> post(~p"/internal/v1/codex/workers/#{worker_id}/events", %{
        "event" => %{
          "event_type" => "worker.event",
          "payload" => %{
            "source" => "autopilot-ios",
            "method" => "ios/handshake",
            "handshake_id" => handshake_id,
            "device_id" => "device_test",
            "occurred_at" => "2026-02-20T00:00:00Z"
          }
        }
      })

    assert %{
             "data" => %{
               "worker_id" => ^worker_id,
               "event_type" => "worker.event",
               "seq" => ios_seq
             }
           } = json_response(ios_ingest_conn, 202)

    ack_ingest_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 910)
      |> post(~p"/internal/v1/codex/workers/#{worker_id}/events", %{
        "event" => %{
          "event_type" => "worker.event",
          "payload" => %{
            "source" => "autopilot-desktop",
            "method" => "desktop/handshake_ack",
            "handshake_id" => handshake_id,
            "desktop_session_id" => "session_42",
            "occurred_at" => "2026-02-20T00:00:02Z"
          }
        }
      })

    assert %{
             "data" => %{
               "worker_id" => ^worker_id,
               "event_type" => "worker.event",
               "seq" => ack_seq
             }
           } = json_response(ack_ingest_conn, 202)

    assert ack_seq > ios_seq

    stream_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 910)
      |> get(~p"/internal/v1/codex/workers/#{worker_id}/stream", %{
        "cursor" => max(ios_seq - 1, 0),
        "tail_ms" => 50
      })

    assert stream_conn.status == 200
    assert List.first(get_resp_header(stream_conn, "content-type")) =~ "text/event-stream"
    assert stream_conn.resp_body =~ "\"method\":\"ios/handshake\""
    assert stream_conn.resp_body =~ "\"method\":\"desktop/handshake_ack\""
    assert stream_conn.resp_body =~ "\"handshake_id\":\"#{handshake_id}\""
  end

  test "events endpoint returns conflict when worker is stopped", %{conn: conn} do
    worker_id = unique_id("codexw")

    conn
    |> put_internal_auth(user_id: 908)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => worker_id})
    |> json_response(202)

    conn
    |> recycle()
    |> put_internal_auth(user_id: 908)
    |> post(~p"/internal/v1/codex/workers/#{worker_id}/stop", %{"reason" => "done"})
    |> json_response(202)

    stopped_event_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 908)
      |> post(~p"/internal/v1/codex/workers/#{worker_id}/events", %{
        "event" => %{
          "event_type" => "worker.event",
          "payload" => %{"source" => "desktop", "method" => "turn/started"}
        }
      })

    assert %{"error" => %{"code" => "conflict"}} = json_response(stopped_event_conn, 409)
  end

  test "event ingest propagates request correlation ids into projection telemetry", %{conn: conn} do
    worker_id = unique_id("codexw")

    conn
    |> put_internal_auth(user_id: 909)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => worker_id})
    |> json_response(202)

    telemetry_ref = "codex-worker-correlation-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        telemetry_ref,
        [:openagents_runtime, :convex, :projection, :write],
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:projection_write_telemetry, measurements, metadata})
        end,
        self()
      )

    on_exit(fn -> :telemetry.detach(telemetry_ref) end)

    traceparent = "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01"
    request_id = "req-corr-#{System.unique_integer([:positive])}"

    ingest_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 909)
      |> put_req_header("traceparent", traceparent)
      |> put_req_header("x-request-id", request_id)
      |> post(~p"/internal/v1/codex/workers/#{worker_id}/events", %{
        "event" => %{
          "event_type" => "worker.event",
          "payload" => %{"source" => "desktop", "method" => "turn/started"}
        }
      })

    assert %{"data" => %{"worker_id" => ^worker_id, "event_type" => "worker.event"}} =
             json_response(ingest_conn, 202)

    [response_request_id] = Plug.Conn.get_resp_header(ingest_conn, "x-request-id")
    assert is_binary(response_request_id) and response_request_id != ""

    assert_receive {:projection_write_telemetry, _measurements, metadata}, 1_000
    assert metadata.projection == "codex_worker_summary"
    assert metadata.x_request_id == request_id
    assert metadata.traceparent == traceparent
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

  test "worker stream enforces ownership", %{conn: conn} do
    worker_id = unique_id("codexw")

    conn
    |> put_internal_auth(user_id: 912)
    |> post(~p"/internal/v1/codex/workers", %{"worker_id" => worker_id})
    |> json_response(202)

    forbidden_stream_conn =
      conn
      |> recycle()
      |> put_internal_auth(user_id: 913)
      |> get(~p"/internal/v1/codex/workers/#{worker_id}/stream", %{"cursor" => 0, "tail_ms" => 10})

    assert %{"error" => %{"code" => "forbidden"}} = json_response(forbidden_stream_conn, 403)
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
