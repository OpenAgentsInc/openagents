defmodule OpenAgentsRuntime.Sync.KhalaFrameTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Sync.KhalaFrame

  test "builds update batch frame with required envelope fields" do
    payload = %{
      "updates" => [
        %{
          "topic" => "runtime.run_summaries",
          "doc_key" => "run:42",
          "doc_version" => 42,
          "watermark" => 42,
          "payload" => %{"status" => "running"}
        }
      ],
      "replay_complete" => false
    }

    assert {:ok, frame} = KhalaFrame.build("runtime.run_summaries", 42, :update_batch, payload)
    assert frame["topic"] == "runtime.run_summaries"
    assert frame["seq"] == 42
    assert frame["kind"] == "KHALA_FRAME_KIND_UPDATE_BATCH"
    assert frame["schema_version"] == 1
    assert is_binary(frame["payload_bytes"])

    assert {:ok, decoded_payload} = KhalaFrame.decode_payload(frame)
    assert decoded_payload == payload
  end

  test "builds error frame for stale cursor payload" do
    payload = %{
      "code" => "stale_cursor",
      "message" => "cursor is older than retention floor",
      "full_resync_required" => true,
      "stale_topics" => [
        %{"topic" => "runtime.run_summaries", "resume_after" => 10, "retention_floor" => 42}
      ]
    }

    assert {:ok, frame} = KhalaFrame.build("runtime.run_summaries", 0, :error, payload)
    assert frame["kind"] == "KHALA_FRAME_KIND_ERROR"
    assert frame["seq"] == 0

    assert {:ok, decoded_payload} = KhalaFrame.decode_payload(frame)
    assert decoded_payload["code"] == "stale_cursor"
    assert decoded_payload["full_resync_required"] == true
  end

  test "rejects invalid frame inputs" do
    assert {:error, :invalid_topic} = KhalaFrame.build("", 1, :update_batch, %{})
    assert {:error, :invalid_kind} = KhalaFrame.build("runtime.run_summaries", 1, :unknown, %{})

    assert {:error, :invalid_payload} =
             KhalaFrame.build("runtime.run_summaries", 1, :update_batch, :bad)

    assert {:error, :invalid_payload} =
             KhalaFrame.decode_payload(%{"payload_bytes" => "not-base64"})
  end
end
