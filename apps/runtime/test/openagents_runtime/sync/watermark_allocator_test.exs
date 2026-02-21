defmodule OpenAgentsRuntime.Sync.WatermarkAllocatorTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Sync.WatermarkAllocator

  @moduletag :sync

  test "allocates monotonic watermarks for an existing topic" do
    assert {:ok, 1} = WatermarkAllocator.next("runtime.run_summaries")
    assert {:ok, 2} = WatermarkAllocator.next("runtime.run_summaries")
    assert {:ok, 3} = WatermarkAllocator.next("runtime.run_summaries")
  end

  test "allocates unique watermarks under concurrency" do
    watermarks =
      1..100
      |> Task.async_stream(
        fn _ -> WatermarkAllocator.next("runtime.codex_worker_summaries") end,
        max_concurrency: 20,
        ordered: false,
        timeout: 5_000
      )
      |> Enum.map(fn {:ok, {:ok, watermark}} -> watermark end)
      |> Enum.sort()

    assert watermarks == Enum.to_list(1..100)
  end

  test "creates missing topic rows and starts from watermark 1" do
    dynamic_topic = "runtime.custom_debug_topic"

    assert {:ok, 1} = WatermarkAllocator.next(dynamic_topic)
    assert {:ok, 2} = WatermarkAllocator.next(dynamic_topic)
  end

  test "rejects empty topic values" do
    assert {:error, :invalid_topic} = WatermarkAllocator.next("   ")
  end
end
