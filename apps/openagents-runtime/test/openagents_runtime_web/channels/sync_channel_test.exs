defmodule OpenAgentsRuntimeWeb.SyncChannelTest do
  use OpenAgentsRuntime.DataCase, async: false

  import Phoenix.ChannelTest
  import OpenAgentsRuntimeWeb.AuthHelpers

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.ConnectionTracker
  alias OpenAgentsRuntime.Sync.Notifier
  alias OpenAgentsRuntime.Sync.RetentionJob
  alias OpenAgentsRuntime.Sync.StreamEvent
  alias OpenAgentsRuntimeWeb.SyncChannel
  alias OpenAgentsRuntimeWeb.SyncSocket

  @endpoint OpenAgentsRuntimeWeb.Endpoint

  @run_topic "runtime.run_summaries"
  @worker_topic "runtime.codex_worker_summaries"
  @worker_events_topic "runtime.codex_worker_events"

  @connection_event [:openagents_runtime, :sync, :socket, :connection]
  @heartbeat_event [:openagents_runtime, :sync, :socket, :heartbeat]
  @reconnect_event [:openagents_runtime, :sync, :socket, :reconnect]
  @timeout_event [:openagents_runtime, :sync, :socket, :timeout]
  @lag_event [:openagents_runtime, :sync, :replay, :lag]
  @catchup_event [:openagents_runtime, :sync, :replay, :catchup]

  setup do
    ConnectionTracker.reset_for_tests()
    :ok
  end

  test "socket rejects missing token" do
    assert :error = connect(SyncSocket, %{})
  end

  test "socket accepts sync jwt signed by rotated kid" do
    previous_sync_auth = Application.get_env(:openagents_runtime, :khala_sync_auth, [])

    Application.put_env(:openagents_runtime, :khala_sync_auth,
      issuer: "https://openagents.test",
      audience: "openagents-sync-test",
      claims_version: "oa_sync_claims_v1",
      allowed_algs: ["HS256"],
      hs256_keys: %{
        "sync-auth-test-v1" => "sync-test-signing-key",
        "sync-auth-rotated-v2" => "sync-rotated-signing-key"
      }
    )

    on_exit(fn ->
      Application.put_env(:openagents_runtime, :khala_sync_auth, previous_sync_auth)
    end)

    token =
      valid_sync_jwt(
        kid: "sync-auth-rotated-v2",
        key: "sync-rotated-signing-key",
        oa_sync_scopes: [@run_topic]
      )

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, _socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")
  end

  test "socket rejects sync jwt with unknown kid" do
    token =
      valid_sync_jwt(
        kid: "sync-auth-unknown-v3",
        key: "sync-unknown-signing-key",
        oa_sync_scopes: [@run_topic]
      )

    assert :error = connect(SyncSocket, %{"token" => token})
  end

  test "authenticated client can join and subscribe to allowed topics" do
    token =
      valid_sync_jwt(
        oa_org_id: "org_123",
        oa_sync_scopes: [@run_topic]
      )

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})

    assert {:ok, reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")
    assert reply["allowed_topics"] == [@run_topic]

    ref = push(socket, "sync:subscribe", %{"topics" => [@run_topic]})

    assert_reply ref, :ok, %{
      "topics" => [@run_topic],
      "current_watermarks" => [
        %{"topic" => @run_topic, "watermark" => 0}
      ]
    }
  end

  test "authenticated client can subscribe to codex worker events topic" do
    token =
      valid_sync_jwt(
        oa_org_id: "org_123",
        oa_sync_scopes: [@worker_events_topic]
      )

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")
    assert reply["allowed_topics"] == [@worker_events_topic]

    ref = push(socket, "sync:subscribe", %{"topics" => [@worker_events_topic]})

    assert_reply ref, :ok, %{
      "topics" => [@worker_events_topic],
      "current_watermarks" => [
        %{"topic" => @worker_events_topic, "watermark" => 0}
      ]
    }
  end

  test "subscribe rejects unauthorized topics" do
    token =
      valid_sync_jwt(
        oa_org_id: "org_123",
        oa_sync_scopes: [@run_topic]
      )

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    ref = push(socket, "sync:subscribe", %{"topics" => [@worker_topic]})

    assert_reply ref, :error, %{
      "code" => "forbidden_topic",
      "forbidden_topics" => [@worker_topic]
    }
  end

  test "subscribe validates malformed payload" do
    token = valid_sync_jwt(oa_sync_scopes: [@run_topic])

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    ref = push(socket, "sync:subscribe", %{"topics" => []})
    assert_reply ref, :error, %{"code" => "bad_subscription"}
  end

  test "subscribe replays events after resume watermark and streams live updates" do
    insert_stream_event(@run_topic, 1, %{"value" => "one"})
    insert_stream_event(@run_topic, 2, %{"value" => "two"})
    insert_stream_event(@run_topic, 3, %{"value" => "three"})

    token = valid_sync_jwt(oa_sync_scopes: [@run_topic])

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    ref =
      push(socket, "sync:subscribe", %{
        "topics" => [@run_topic],
        "resume_after" => [
          %{"topic" => @run_topic, "watermark" => 1}
        ]
      })

    assert_push "sync:update_batch", %{
      "updates" => replay_updates,
      "replay_complete" => true,
      "head_watermarks" => [%{"topic" => @run_topic, "watermark" => 3}]
    }

    assert Enum.map(replay_updates, & &1["watermark"]) == [2, 3]

    assert_reply ref, :ok, %{
      "topics" => [@run_topic],
      "current_watermarks" => [
        %{"topic" => @run_topic, "watermark" => 3}
      ]
    }

    insert_stream_event(@run_topic, 4, %{"value" => "four"})
    :ok = Notifier.broadcast_stream_event(@run_topic, 4)

    assert_push "sync:update_batch", %{
      "updates" => [live_update],
      "replay_complete" => true,
      "head_watermarks" => [%{"topic" => @run_topic, "watermark" => 4}]
    }

    assert live_update["watermark"] == 4
    assert live_update["payload"]["value"] == "four"
  end

  test "reconnect with resume watermark replays only missing events" do
    insert_stream_event(@run_topic, 1, %{"value" => "one"})
    insert_stream_event(@run_topic, 2, %{"value" => "two"})
    insert_stream_event(@run_topic, 3, %{"value" => "three"})

    token = valid_sync_jwt(oa_sync_scopes: [@run_topic])
    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    first_ref = push(socket, "sync:subscribe", %{"topics" => [@run_topic]})

    assert_push "sync:update_batch", %{
      "updates" => first_updates,
      "head_watermarks" => [%{"topic" => @run_topic, "watermark" => 3}]
    }

    assert Enum.map(first_updates, & &1["watermark"]) == [1, 2, 3]
    assert_reply first_ref, :ok, _first_reply

    insert_stream_event(@run_topic, 4, %{"value" => "four"})
    insert_stream_event(@run_topic, 5, %{"value" => "five"})

    reconnect_token = valid_sync_jwt(oa_sync_scopes: [@run_topic])
    assert {:ok, reconnect_socket} = connect(SyncSocket, %{"token" => reconnect_token})

    assert {:ok, _reply, reconnect_socket} =
             subscribe_and_join(reconnect_socket, SyncChannel, "sync:v1")

    reconnect_ref =
      push(reconnect_socket, "sync:subscribe", %{
        "topics" => [@run_topic],
        "resume_after" => %{@run_topic => 3}
      })

    assert_push "sync:update_batch", %{
      "updates" => reconnect_updates,
      "head_watermarks" => [%{"topic" => @run_topic, "watermark" => 5}]
    }

    assert Enum.map(reconnect_updates, & &1["watermark"]) == [4, 5]

    assert_reply reconnect_ref, :ok, %{
      "current_watermarks" => [%{"topic" => @run_topic, "watermark" => 5}]
    }
  end

  test "forced socket drop reconnect resumes without gaps" do
    insert_stream_event(@run_topic, 1, %{"value" => "one"})
    insert_stream_event(@run_topic, 2, %{"value" => "two"})
    insert_stream_event(@run_topic, 3, %{"value" => "three"})

    token = valid_sync_jwt(oa_sync_scopes: [@run_topic])
    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    subscribe_ref = push(socket, "sync:subscribe", %{"topics" => [@run_topic]})

    assert_push "sync:update_batch", %{
      "updates" => initial_updates,
      "head_watermarks" => [%{"topic" => @run_topic, "watermark" => 3}]
    }

    assert Enum.map(initial_updates, & &1["watermark"]) == [1, 2, 3]
    assert_reply subscribe_ref, :ok, _subscribe_reply

    Process.unlink(socket.channel_pid)
    :ok = close(socket)

    insert_stream_event(@run_topic, 4, %{"value" => "four"})
    insert_stream_event(@run_topic, 5, %{"value" => "five"})
    insert_stream_event(@run_topic, 6, %{"value" => "six"})

    reconnect_token = valid_sync_jwt(oa_sync_scopes: [@run_topic])
    assert {:ok, reconnect_socket} = connect(SyncSocket, %{"token" => reconnect_token})

    assert {:ok, _reply, reconnect_socket} =
             subscribe_and_join(reconnect_socket, SyncChannel, "sync:v1")

    reconnect_ref =
      push(reconnect_socket, "sync:subscribe", %{
        "topics" => [@run_topic],
        "resume_after" => %{@run_topic => 3}
      })

    assert_push "sync:update_batch", %{
      "updates" => reconnect_updates,
      "head_watermarks" => [%{"topic" => @run_topic, "watermark" => 6}]
    }

    assert Enum.map(reconnect_updates, & &1["watermark"]) == [4, 5, 6]

    assert_reply reconnect_ref, :ok, %{
      "current_watermarks" => [%{"topic" => @run_topic, "watermark" => 6}]
    }
  end

  test "subscribe returns stale_cursor after retention purge simulation" do
    now = ~U[2026-02-20 12:00:00.000000Z]

    insert_stream_event(@run_topic, 1, %{"value" => "old"}, DateTime.add(now, -7_200, :second))
    insert_stream_event(@run_topic, 2, %{"value" => "fresh"}, DateTime.add(now, -60, :second))

    summary = RetentionJob.run_once(now: now, horizon_seconds: 3_600, batch_size: 10)
    assert summary.oldest_retained[@run_topic] == 2

    token = valid_sync_jwt(oa_sync_scopes: [@run_topic])
    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    ref =
      push(socket, "sync:subscribe", %{
        "topics" => [@run_topic],
        "resume_after" => %{@run_topic => 0}
      })

    assert_push "sync:error", %{
      "code" => "stale_cursor",
      "full_resync_required" => true,
      "stale_topics" => [
        %{
          "topic" => @run_topic,
          "resume_after" => 0,
          "retention_floor" => 1
        }
      ]
    }

    assert_reply ref, :error, %{
      "code" => "stale_cursor",
      "full_resync_required" => true,
      "stale_topics" => [
        %{
          "topic" => @run_topic,
          "resume_after" => 0,
          "retention_floor" => 1
        }
      ]
    }
  end

  test "subscribe emits reconnect/lag/catch-up telemetry" do
    insert_stream_event(@run_topic, 1, %{"value" => "one"})
    insert_stream_event(@run_topic, 2, %{"value" => "two"})
    insert_stream_event(@run_topic, 3, %{"value" => "three"})

    telemetry_refs = attach_sync_telemetry(self())

    on_exit(fn ->
      Enum.each(telemetry_refs, &:telemetry.detach/1)
    end)

    token = valid_sync_jwt(oa_sync_scopes: [@run_topic])
    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    assert_receive {:sync_connection, connection_measurements, connection_metadata}
    assert connection_measurements.active_connections >= 1
    assert connection_metadata.action == "connect"

    ref =
      push(socket, "sync:subscribe", %{
        "topics" => [@run_topic],
        "resume_after" => %{@run_topic => 1}
      })

    assert_push "sync:update_batch", %{"updates" => updates}
    assert Enum.map(updates, & &1["watermark"]) == [2, 3]

    assert_reply ref, :ok, _reply

    assert_receive {:sync_reconnect, %{count: 1}, %{status: "ok"}}
    assert_receive {:sync_lag, lag_measurements, lag_metadata}
    assert lag_measurements.lag_events >= 0
    assert lag_metadata.event_type == @run_topic
    assert_receive {:sync_catchup, catchup_measurements, %{status: "ok"}}
    assert catchup_measurements.duration_ms >= 0
  end

  test "heartbeat roundtrip and timeout handling" do
    old_interval = Application.get_env(:openagents_runtime, :khala_sync_heartbeat_interval_ms)
    old_timeout = Application.get_env(:openagents_runtime, :khala_sync_heartbeat_timeout_ms)

    Application.put_env(:openagents_runtime, :khala_sync_heartbeat_interval_ms, 10)
    Application.put_env(:openagents_runtime, :khala_sync_heartbeat_timeout_ms, 20)

    on_exit(fn ->
      Application.put_env(:openagents_runtime, :khala_sync_heartbeat_interval_ms, old_interval)
      Application.put_env(:openagents_runtime, :khala_sync_heartbeat_timeout_ms, old_timeout)
    end)

    telemetry_refs = attach_sync_telemetry(self())

    on_exit(fn ->
      Enum.each(telemetry_refs, &:telemetry.detach/1)
    end)

    token = valid_sync_jwt(oa_sync_scopes: [@run_topic])
    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    ref = push(socket, "sync:subscribe", %{"topics" => [@run_topic]})
    assert_reply ref, :ok, _reply

    assert_push "sync:heartbeat", %{"watermarks" => _watermarks}
    assert_receive {:sync_heartbeat, %{count: 1}, %{status: "server"}}

    heartbeat_ref = push(socket, "sync:heartbeat", %{})

    assert_push "sync:heartbeat", %{"watermarks" => _watermarks}
    assert_reply heartbeat_ref, :ok, %{"watermarks" => _watermarks}
    assert_receive {:sync_heartbeat, %{count: 1}, %{status: "client"}}

    Process.sleep(45)

    assert_receive {:sync_timeout, %{count: 1}, %{status: "timeout"}}, 200
  end

  defp insert_stream_event(
         topic,
         watermark,
         payload,
         inserted_at \\ DateTime.utc_now() |> DateTime.truncate(:microsecond)
       ) do
    Repo.insert_all(StreamEvent, [
      %{
        topic: topic,
        watermark: watermark,
        doc_key: "#{topic}:#{watermark}",
        doc_version: watermark,
        payload: payload,
        payload_hash: :crypto.hash(:sha256, Jason.encode!(payload)),
        inserted_at: inserted_at
      }
    ])
  end

  defp attach_sync_telemetry(test_pid) do
    events = [
      {"sync-connection-#{System.unique_integer([:positive])}", @connection_event,
       :sync_connection},
      {"sync-heartbeat-#{System.unique_integer([:positive])}", @heartbeat_event, :sync_heartbeat},
      {"sync-reconnect-#{System.unique_integer([:positive])}", @reconnect_event, :sync_reconnect},
      {"sync-timeout-#{System.unique_integer([:positive])}", @timeout_event, :sync_timeout},
      {"sync-lag-#{System.unique_integer([:positive])}", @lag_event, :sync_lag},
      {"sync-catchup-#{System.unique_integer([:positive])}", @catchup_event, :sync_catchup}
    ]

    Enum.map(events, fn {handler_id, event_name, message_tag} ->
      :ok =
        :telemetry.attach(
          handler_id,
          event_name,
          fn _event, measurements, metadata, pid ->
            send(pid, {message_tag, measurements, metadata})
          end,
          test_pid
        )

      handler_id
    end)
  end
end
