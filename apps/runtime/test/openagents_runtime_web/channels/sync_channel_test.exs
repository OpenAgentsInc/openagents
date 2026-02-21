defmodule OpenAgentsRuntimeWeb.SyncChannelTest do
  use OpenAgentsRuntime.DataCase, async: false

  import Phoenix.ChannelTest
  import OpenAgentsRuntimeWeb.AuthHelpers

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.ConnectionTracker
  alias OpenAgentsRuntime.Sync.Notifier
  alias OpenAgentsRuntime.Sync.RetentionJob
  alias OpenAgentsRuntime.Sync.SessionRevocation
  alias OpenAgentsRuntime.Sync.StreamEvent
  alias OpenAgentsRuntimeWeb.SyncChannel
  alias OpenAgentsRuntimeWeb.SyncSocket

  @endpoint OpenAgentsRuntimeWeb.Endpoint

  @run_topic "runtime.run_summaries"
  @worker_topic "runtime.codex_worker_summaries"
  @worker_events_topic "runtime.codex_worker_events"

  @connection_event [:openagents_runtime, :sync, :socket, :connection]
  @auth_event [:openagents_runtime, :sync, :socket, :auth]
  @heartbeat_event [:openagents_runtime, :sync, :socket, :heartbeat]
  @reconnect_event [:openagents_runtime, :sync, :socket, :reconnect]
  @timeout_event [:openagents_runtime, :sync, :socket, :timeout]
  @revocation_event [:openagents_runtime, :sync, :socket, :revocation]
  @queue_event [:openagents_runtime, :sync, :socket, :queue]
  @slow_consumer_event [:openagents_runtime, :sync, :socket, :slow_consumer]
  @lag_event [:openagents_runtime, :sync, :replay, :lag]
  @catchup_event [:openagents_runtime, :sync, :replay, :catchup]

  setup do
    ConnectionTracker.reset_for_tests()
    SessionRevocation.reset_for_tests()
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

  test "socket auth emits telemetry for successful and rejected auth" do
    auth_ref = "sync-auth-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        auth_ref,
        @auth_event,
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:sync_auth, measurements, metadata})
        end,
        self()
      )

    on_exit(fn ->
      :telemetry.detach(auth_ref)
    end)

    valid_token = valid_sync_jwt(oa_sync_scopes: [@run_topic])
    assert {:ok, _socket} = connect(SyncSocket, %{"token" => valid_token})

    assert_receive {:sync_auth, %{count: 1}, %{status: "ok", reason_class: "authorized"}}

    invalid_token =
      valid_sync_jwt(
        kid: "sync-auth-unknown-v3",
        key: "sync-unknown-signing-key",
        oa_sync_scopes: [@run_topic]
      )

    assert :error = connect(SyncSocket, %{"token" => invalid_token})

    assert_receive {:sync_auth, %{count: 1}, %{status: "error", reason_class: "unknown_kid"}}

    assert :error = connect(SyncSocket, %{})
    assert_receive {:sync_auth, %{count: 1}, %{status: "error", reason_class: "missing_token"}}
  end

  test "khala compatibility gate rejects unsupported client build at join" do
    previous_sync_auth = Application.get_env(:openagents_runtime, :khala_sync_auth, [])

    Application.put_env(
      :openagents_runtime,
      :khala_sync_auth,
      Keyword.merge(previous_sync_auth,
        compat_enforced: true,
        compat_protocol_version: "khala.ws.v1",
        compat_min_client_build_id: "20260221T120000Z",
        compat_max_client_build_id: "20260221T180000Z",
        compat_min_schema_version: 1,
        compat_max_schema_version: 1
      )
    )

    auth_ref = "sync-auth-compat-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        auth_ref,
        @auth_event,
        fn _event_name, measurements, metadata, test_pid ->
          send(test_pid, {:sync_auth, measurements, metadata})
        end,
        self()
      )

    on_exit(fn ->
      :telemetry.detach(auth_ref)
      Application.put_env(:openagents_runtime, :khala_sync_auth, previous_sync_auth)
    end)

    token = valid_sync_jwt(oa_sync_scopes: [@run_topic])

    assert {:ok, socket} =
             connect(SyncSocket, %{
               "token" => token,
               "client" => "autopilot-ios",
               "client_build_id" => "20260221T110000Z",
               "protocol_version" => "khala.ws.v1",
               "schema_version" => "1"
             })

    assert_receive {:sync_auth, %{count: 1},
                    %{
                      status: "error",
                      reason_class: "upgrade_required",
                      surface: "khala_websocket",
                      client: "autopilot-ios",
                      client_build_id: "20260221T110000Z"
                    }}

    assert {:error,
            %{
              "code" => "upgrade_required",
              "upgrade_required" => true,
              "surface" => "khala_websocket",
              "min_client_build_id" => "20260221T120000Z"
            }} = subscribe_and_join(socket, SyncChannel, "sync:v1")
  end

  test "khala compatibility gate allows supported client metadata" do
    previous_sync_auth = Application.get_env(:openagents_runtime, :khala_sync_auth, [])

    Application.put_env(
      :openagents_runtime,
      :khala_sync_auth,
      Keyword.merge(previous_sync_auth,
        compat_enforced: true,
        compat_protocol_version: "khala.ws.v1",
        compat_min_client_build_id: "20260221T120000Z",
        compat_max_client_build_id: "20260221T180000Z",
        compat_min_schema_version: 1,
        compat_max_schema_version: 1
      )
    )

    on_exit(fn ->
      Application.put_env(:openagents_runtime, :khala_sync_auth, previous_sync_auth)
    end)

    token = valid_sync_jwt(oa_sync_scopes: [@run_topic])

    assert {:ok, socket} =
             connect(SyncSocket, %{
               "token" => token,
               "client_build_id" => "20260221T130000Z",
               "protocol_version" => "khala.ws.v1",
               "schema_version" => "1"
             })

    assert {:ok, _reply, _socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")
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

    assert_push "sync:frame", replay_frame
    assert replay_frame["topic"] == @run_topic
    assert replay_frame["seq"] == 3
    assert replay_frame["kind"] == "KHALA_FRAME_KIND_UPDATE_BATCH"
    assert replay_frame["schema_version"] == 1
    assert {:ok, replay_payload} = decode_frame_payload(replay_frame)
    assert replay_payload["replay_complete"] == true

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

    assert_push "sync:frame", live_frame
    assert live_frame["topic"] == @run_topic
    assert live_frame["seq"] == 4
    assert live_frame["kind"] == "KHALA_FRAME_KIND_UPDATE_BATCH"
    assert live_frame["schema_version"] == 1
    assert {:ok, live_payload} = decode_frame_payload(live_frame)
    assert %{"updates" => [%{"watermark" => 4}]} = live_payload

    assert live_update["watermark"] == 4
    assert live_update["payload"]["value"] == "four"
  end

  @tag :chaos_drill
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

  @tag :chaos_drill
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

  @tag :chaos_drill
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
      "reason_codes" => ["retention_floor_breach"],
      "snapshot_plan" => snapshot_plan,
      "stale_topics" => [
        %{
          "topic" => @run_topic,
          "reason" => "retention_floor_breach",
          "resume_after" => 0,
          "retention_floor" => 1,
          "head_watermark" => 2
        }
      ]
    }

    assert snapshot_plan["format"] == "openagents.sync.snapshot.v1"

    assert [%{"topic" => @run_topic, "head_watermark" => 2, "snapshot" => snapshot_metadata}] =
             snapshot_plan["topics"]

    assert snapshot_metadata["format"] == "openagents.sync.snapshot.v1"
    assert snapshot_metadata["source_table"] == "runtime.sync_run_summaries"

    assert_push "sync:frame", stale_frame
    assert stale_frame["kind"] == "KHALA_FRAME_KIND_ERROR"
    assert stale_frame["topic"] == @run_topic
    assert stale_frame["seq"] == 0
    assert stale_frame["schema_version"] == 1
    assert {:ok, stale_payload} = decode_frame_payload(stale_frame)
    assert stale_payload["code"] == "stale_cursor"
    assert stale_payload["full_resync_required"] == true

    assert_reply ref, :error, %{
      "code" => "stale_cursor",
      "full_resync_required" => true,
      "reason_codes" => ["retention_floor_breach"],
      "snapshot_plan" => %{
        "format" => "openagents.sync.snapshot.v1"
      },
      "stale_topics" => [
        %{
          "topic" => @run_topic,
          "reason" => "retention_floor_breach",
          "resume_after" => 0,
          "retention_floor" => 1,
          "head_watermark" => 2
        }
      ]
    }
  end

  test "subscribe returns stale_cursor when replay budget is exceeded for topic tier" do
    previous_topic_policies =
      Application.get_env(:openagents_runtime, :khala_sync_topic_policies, %{})

    Application.put_env(
      :openagents_runtime,
      :khala_sync_topic_policies,
      Map.merge(previous_topic_policies, %{
        @run_topic => %{
          topic_class: "durable_summary",
          qos_tier: "warm",
          replay_budget_events: 2,
          retention_seconds: 604_800,
          compaction_mode: "tail_prune_with_snapshot_rehydrate",
          snapshot: %{
            enabled: true,
            format: "openagents.sync.snapshot.v1",
            schema_version: 1,
            cadence_seconds: 300,
            source_table: "runtime.sync_run_summaries"
          }
        }
      })
    )

    on_exit(fn ->
      Application.put_env(
        :openagents_runtime,
        :khala_sync_topic_policies,
        previous_topic_policies
      )
    end)

    insert_stream_event(@run_topic, 1, %{"value" => "one"})
    insert_stream_event(@run_topic, 2, %{"value" => "two"})
    insert_stream_event(@run_topic, 3, %{"value" => "three"})
    insert_stream_event(@run_topic, 4, %{"value" => "four"})

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
      "reason_codes" => ["replay_budget_exceeded"],
      "stale_topics" => [
        %{
          "topic" => @run_topic,
          "reason" => "replay_budget_exceeded",
          "qos_tier" => "warm",
          "resume_after" => 0,
          "head_watermark" => 4,
          "replay_lag" => 4,
          "replay_budget_events" => 2
        }
      ]
    }

    assert_reply ref, :error, %{
      "code" => "stale_cursor",
      "reason_codes" => ["replay_budget_exceeded"],
      "stale_topics" => [
        %{
          "topic" => @run_topic,
          "reason" => "replay_budget_exceeded",
          "replay_budget_events" => 2
        }
      ]
    }
  end

  @tag :chaos_drill
  test "reconnect with expired token fails and fresh token resumes stream" do
    now = System.system_time(:second)

    insert_stream_event(@run_topic, 1, %{"value" => "one"})
    insert_stream_event(@run_topic, 2, %{"value" => "two"})
    insert_stream_event(@run_topic, 3, %{"value" => "three"})

    token = valid_sync_jwt(oa_sync_scopes: [@run_topic], now: now, ttl_seconds: 900)
    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    subscribe_ref = push(socket, "sync:subscribe", %{"topics" => [@run_topic]})

    assert_push "sync:update_batch", %{
      "updates" => initial_updates,
      "head_watermarks" => [%{"topic" => @run_topic, "watermark" => 3}]
    }

    assert Enum.map(initial_updates, & &1["watermark"]) == [1, 2, 3]
    assert_reply subscribe_ref, :ok, _reply

    Process.unlink(socket.channel_pid)
    :ok = close(socket)

    insert_stream_event(@run_topic, 4, %{"value" => "four"})
    insert_stream_event(@run_topic, 5, %{"value" => "five"})

    expired_token =
      valid_sync_jwt(
        oa_sync_scopes: [@run_topic],
        now: now - 1_200,
        ttl_seconds: 300
      )

    assert :error = connect(SyncSocket, %{"token" => expired_token})

    fresh_token = valid_sync_jwt(oa_sync_scopes: [@run_topic], now: now + 1, ttl_seconds: 900)
    assert {:ok, reconnect_socket} = connect(SyncSocket, %{"token" => fresh_token})

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

  @tag :chaos_drill
  test "revoked session disconnects live socket and reconnect requires reauth" do
    telemetry_refs = attach_sync_telemetry(self())

    on_exit(fn ->
      Enum.each(telemetry_refs, &:telemetry.detach/1)
    end)

    session_id = "sess-live-revoke"
    device_id = "ios-live-revoke"

    token =
      valid_sync_jwt(
        oa_sync_scopes: [@run_topic],
        oa_session_id: session_id,
        oa_device_id: device_id
      )

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    subscribe_ref = push(socket, "sync:subscribe", %{"topics" => [@run_topic]})
    assert_reply subscribe_ref, :ok, _reply

    Process.unlink(socket.channel_pid)
    monitor_ref = Process.monitor(socket.channel_pid)

    revoke_result = SessionRevocation.revoke(session_ids: [session_id], reason: "user_requested")
    assert revoke_result.revoked_session_ids == [session_id]

    assert_receive {:DOWN, ^monitor_ref, :process, _pid, _reason}, 500

    assert {:ok, reconnect_socket} = connect(SyncSocket, %{"token" => token})

    assert {:error,
            %{
              "code" => "reauth_required",
              "reauth_required" => true,
              "reason" => "user_requested"
            }} = subscribe_and_join(reconnect_socket, SyncChannel, "sync:v1")

    assert_receive {:sync_revocation, %{count: 1},
                    %{status: "reauth_required", result: "join_denied"}}
  end

  test "revoked session auth emits deterministic reauth_required reason" do
    session_id = "sess-reauth-required"
    device_id = "ios-reauth-required"

    SessionRevocation.revoke(session_ids: [session_id], reason: "security_policy")

    token =
      valid_sync_jwt(
        oa_sync_scopes: [@run_topic],
        oa_session_id: session_id,
        oa_device_id: device_id
      )

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})

    assert {:error,
            %{
              "code" => "reauth_required",
              "reauth_required" => true,
              "reason" => "security_policy"
            }} = subscribe_and_join(socket, SyncChannel, "sync:v1")
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

  test "fair drain scheduling prevents hot topics from starving lower-volume topics" do
    old_batch_size = Application.get_env(:openagents_runtime, :khala_sync_replay_batch_size)

    old_fair_tick =
      Application.get_env(:openagents_runtime, :khala_sync_fair_drain_topics_per_tick)

    Application.put_env(:openagents_runtime, :khala_sync_replay_batch_size, 1)
    Application.put_env(:openagents_runtime, :khala_sync_fair_drain_topics_per_tick, 1)

    on_exit(fn ->
      Application.put_env(:openagents_runtime, :khala_sync_replay_batch_size, old_batch_size)

      Application.put_env(
        :openagents_runtime,
        :khala_sync_fair_drain_topics_per_tick,
        old_fair_tick
      )
    end)

    token =
      valid_sync_jwt(oa_sync_scopes: [@run_topic, @worker_topic])

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    subscribe_ref = push(socket, "sync:subscribe", %{"topics" => [@run_topic, @worker_topic]})
    assert_reply subscribe_ref, :ok, _reply

    Enum.each(1..20, fn watermark ->
      insert_stream_event(@run_topic, watermark, %{"value" => "run-#{watermark}"})
    end)

    insert_stream_event(@worker_topic, 1, %{"value" => "worker-1"})

    :ok = Notifier.broadcast_stream_event(@run_topic, 20)
    :ok = Notifier.broadcast_stream_event(@worker_topic, 1)

    assert_push "sync:update_batch", %{"updates" => [first_update | _rest]}
    assert first_update["topic"] == @run_topic

    assert_push "sync:update_batch", %{"updates" => [second_update | _rest]}
    assert second_update["topic"] == @worker_topic
  end

  test "slow consumer overflow triggers deterministic disconnect policy" do
    old_queue_limit = Application.get_env(:openagents_runtime, :khala_sync_outbound_queue_limit)

    old_fair_tick =
      Application.get_env(:openagents_runtime, :khala_sync_fair_drain_topics_per_tick)

    old_max_strikes =
      Application.get_env(:openagents_runtime, :khala_sync_slow_consumer_max_strikes)

    Application.put_env(:openagents_runtime, :khala_sync_outbound_queue_limit, 1)
    Application.put_env(:openagents_runtime, :khala_sync_fair_drain_topics_per_tick, 1)
    Application.put_env(:openagents_runtime, :khala_sync_slow_consumer_max_strikes, 1)

    telemetry_refs = attach_sync_telemetry(self())

    on_exit(fn ->
      Application.put_env(:openagents_runtime, :khala_sync_outbound_queue_limit, old_queue_limit)

      Application.put_env(
        :openagents_runtime,
        :khala_sync_fair_drain_topics_per_tick,
        old_fair_tick
      )

      Application.put_env(
        :openagents_runtime,
        :khala_sync_slow_consumer_max_strikes,
        old_max_strikes
      )

      Enum.each(telemetry_refs, &:telemetry.detach/1)
    end)

    token =
      valid_sync_jwt(oa_sync_scopes: [@run_topic, @worker_topic])

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    subscribe_ref = push(socket, "sync:subscribe", %{"topics" => [@run_topic, @worker_topic]})
    assert_reply subscribe_ref, :ok, _reply

    Process.unlink(socket.channel_pid)
    monitor_ref = Process.monitor(socket.channel_pid)

    insert_stream_event(@run_topic, 1, %{"value" => "run"})
    insert_stream_event(@worker_topic, 1, %{"value" => "worker"})
    :ok = Notifier.broadcast_stream_event(@run_topic, 1)
    :ok = Notifier.broadcast_stream_event(@worker_topic, 1)

    assert_push "sync:error", %{
      "code" => "slow_consumer",
      "action" => "disconnect",
      "queue_limit" => 1,
      "full_resync_required" => true
    }

    assert_receive {:sync_slow_consumer, %{count: 1},
                    %{reason_class: "queue_overflow", action: "disconnect"}}

    assert_receive {:DOWN, ^monitor_ref, :process, _pid, _reason}, 500
  end

  @tag :chaos_drill
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
      {"sync-revocation-#{System.unique_integer([:positive])}", @revocation_event,
       :sync_revocation},
      {"sync-queue-#{System.unique_integer([:positive])}", @queue_event, :sync_queue},
      {"sync-slow-consumer-#{System.unique_integer([:positive])}", @slow_consumer_event,
       :sync_slow_consumer},
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

  defp decode_frame_payload(%{"payload_bytes" => encoded_payload})
       when is_binary(encoded_payload) do
    with {:ok, decoded_bytes} <- Base.decode64(encoded_payload),
         {:ok, decoded_payload} <- Jason.decode(decoded_bytes) do
      {:ok, decoded_payload}
    else
      _error -> {:error, :invalid_payload}
    end
  end
end
