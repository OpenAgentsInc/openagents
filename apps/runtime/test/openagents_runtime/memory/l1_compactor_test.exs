defmodule OpenAgentsRuntime.Memory.L1CompactorTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Memory.L1Compactor
  alias OpenAgentsRuntime.Memory.TimelineStore
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run

  setup do
    run_id = unique_run_id("compact")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "running",
      owner_user_id: 77,
      latest_seq: 0
    })

    %{run_id: run_id}
  end

  test "compacts raw events into auditable L1 chunk and drops compacted raw window", %{
    run_id: run_id
  } do
    for seq <- 1..3 do
      assert {:ok, _event} =
               TimelineStore.append_raw_event(run_id, %{
                 seq: seq,
                 event_type: "run.delta",
                 event_class: "conversation",
                 payload: %{"delta" => "chunk-#{seq}"}
               })
    end

    assert {:ok, result} =
             L1Compactor.compact_l1(run_id,
               trigger: :scheduled,
               event_class: "conversation",
               model_name: "claude",
               model_version: "4.1",
               artifact_uri: "gs://openagents/compactions/#{run_id}.json"
             )

    assert result.status == :succeeded
    assert result.input_event_count == 3
    assert result.dropped_event_count == 3
    assert is_binary(result.output_chunk_id)

    assert TimelineStore.list_raw_events(run_id, event_class: "conversation") == []

    [chunk] = TimelineStore.list_chunks(run_id, level: 1)
    assert chunk.chunk_id == result.output_chunk_id
    assert chunk.summary["kind"] == "l1_compaction"
    assert chunk.summary["model"]["name"] == "claude"
    assert chunk.storage_uri =~ "gs://openagents/compactions/"

    [compaction] = TimelineStore.list_compactions(run_id)
    assert compaction.status == "succeeded"
    assert compaction.trigger_type == "scheduled"
    assert compaction.input_event_count == 3
    assert compaction.output_chunk_id == result.output_chunk_id
    assert is_binary(compaction.summary_hash)
  end

  test "returns noop and emits noop audit record when no raw events are available", %{
    run_id: run_id
  } do
    assert {:ok, result} = L1Compactor.compact_l1(run_id, trigger: :pressure)
    assert result.status == :noop
    assert result.input_event_count == 0

    [compaction] = TimelineStore.list_compactions(run_id)
    assert compaction.status == "noop"
    assert compaction.trigger_type == "pressure"
    assert compaction.input_event_count == 0
  end

  defp unique_run_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
