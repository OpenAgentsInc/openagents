defmodule OpenAgentsRuntimeWeb.SyncChannelTest do
  use OpenAgentsRuntime.DataCase, async: true

  import Phoenix.ChannelTest

  import OpenAgentsRuntimeWeb.AuthHelpers

  alias OpenAgentsRuntimeWeb.SyncChannel
  alias OpenAgentsRuntimeWeb.SyncSocket

  @endpoint OpenAgentsRuntimeWeb.Endpoint

  test "socket rejects missing token" do
    assert :error = connect(SyncSocket, %{})
  end

  test "authenticated client can join and subscribe to allowed topics" do
    token =
      valid_signature_token(
        oa_org_id: "org_123",
        oa_sync_scopes: ["runtime.run_summaries"]
      )

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})

    assert {:ok, reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")
    assert reply["allowed_topics"] == ["runtime.run_summaries"]

    ref = push(socket, "sync:subscribe", %{"topics" => ["runtime.run_summaries"]})

    assert_reply ref, :ok, %{
      "topics" => ["runtime.run_summaries"],
      "current_watermarks" => []
    }
  end

  test "subscribe rejects unauthorized topics" do
    token =
      valid_signature_token(
        oa_org_id: "org_123",
        oa_sync_scopes: ["runtime.run_summaries"]
      )

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    ref =
      push(socket, "sync:subscribe", %{"topics" => ["runtime.codex_worker_summaries"]})

    assert_reply ref, :error, %{
      "code" => "forbidden_topic",
      "forbidden_topics" => ["runtime.codex_worker_summaries"]
    }
  end

  test "subscribe validates malformed payload" do
    token = valid_signature_token(oa_sync_scopes: ["runtime.run_summaries"])

    assert {:ok, socket} = connect(SyncSocket, %{"token" => token})
    assert {:ok, _reply, socket} = subscribe_and_join(socket, SyncChannel, "sync:v1")

    ref = push(socket, "sync:subscribe", %{"topics" => []})
    assert_reply ref, :error, %{"code" => "bad_subscription"}
  end
end
