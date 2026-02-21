defmodule OpenAgentsRuntime.Memory.RollupTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Memory.Rollup
  alias OpenAgentsRuntime.Memory.TimelineStore
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run

  setup do
    run_id = unique_run_id("rollup")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "running",
      owner_user_id: 66,
      latest_seq: 0
    })

    now = DateTime.utc_now() |> DateTime.truncate(:second)

    for index <- 1..6 do
      started_at = DateTime.add(now, -(600 - index * 10), :second)
      ended_at = DateTime.add(started_at, 9, :second)

      assert {:ok, _chunk} =
               TimelineStore.insert_chunk(run_id, %{
                 chunk_id: "l1_#{index}",
                 level: 1,
                 event_class: "conversation",
                 retention_class: "durable",
                 window_started_at: started_at,
                 window_ended_at: ended_at,
                 source_event_start_seq: index * 10,
                 source_event_end_seq: index * 10 + 9,
                 summary: %{"text" => "l1 summary #{index}"},
                 token_count: 20 + index
               })
    end

    %{run_id: run_id}
  end

  test "rollup_l2 creates deterministic rollup chunk and idempotent replay", %{run_id: run_id} do
    assert {:ok, first} =
             Rollup.rollup_l2(run_id,
               chunk_count: 6,
               trigger: :scheduled,
               event_class: "conversation"
             )

    assert first.status == :succeeded
    assert first.source_count == 6
    assert first.idempotent_replay == false
    assert is_binary(first.output_chunk_id)

    assert {:ok, replay} =
             Rollup.rollup_l2(run_id,
               chunk_count: 6,
               trigger: :scheduled,
               event_class: "conversation"
             )

    assert replay.idempotent_replay == true
    assert replay.output_chunk_id == first.output_chunk_id

    [rollup] = TimelineStore.list_rollups(run_id, target_level: 2)
    assert rollup.target_level == 2
    assert rollup.source_level == 1
    assert rollup.status == "succeeded"
    assert rollup.output_chunk_id == first.output_chunk_id
  end

  test "rollup_l3 creates level-3 chunk from existing level-2 chunks", %{run_id: run_id} do
    assert {:ok, l2} = Rollup.rollup_l2(run_id, chunk_count: 6, trigger: :manual)
    assert l2.status == :succeeded

    # Seed extra L2 chunk so L3 has at least 2 source chunks.
    l2_chunk = TimelineStore.get_chunk(run_id, l2.output_chunk_id)

    assert {:ok, _extra_l2} =
             TimelineStore.insert_chunk(run_id, %{
               chunk_id: "l2_extra",
               level: 2,
               event_class: "conversation",
               retention_class: "archive",
               window_started_at: l2_chunk.window_started_at,
               window_ended_at: l2_chunk.window_ended_at,
               source_chunk_ids: l2_chunk.source_chunk_ids || [],
               summary: %{"text" => "extra l2"},
               token_count: 90
             })

    assert {:ok, l3} = Rollup.rollup_l3(run_id, chunk_count: 2, trigger: :manual)
    assert l3.status == :succeeded
    assert is_binary(l3.output_chunk_id)

    chunk = TimelineStore.get_chunk(run_id, l3.output_chunk_id)
    assert chunk.level == 3
    assert chunk.summary["kind"] == "l3_rollup"
  end

  test "expand_chunk enforces authorization and bounded expansion", %{run_id: run_id} do
    assert {:ok, l2} = Rollup.rollup_l2(run_id, chunk_count: 6)

    assert {:error, :expansion_not_allowed} =
             Rollup.expand_chunk(run_id, l2.output_chunk_id,
               authorization_mode: "deny",
               max_depth: 3
             )

    assert {:ok, expanded} =
             Rollup.expand_chunk(run_id, l2.output_chunk_id,
               authorization_mode: "interactive",
               max_depth: 1,
               max_items: 4
             )

    assert expanded["chunk_id"] == l2.output_chunk_id
    assert expanded["max_depth"] == 1
    assert expanded["max_items"] == 4
    assert is_map(expanded["tree"])
  end

  defp unique_run_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
