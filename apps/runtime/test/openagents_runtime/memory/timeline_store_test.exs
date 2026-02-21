defmodule OpenAgentsRuntime.Memory.TimelineStoreTest do
  use OpenAgentsRuntime.DataCase, async: false

  alias OpenAgentsRuntime.Memory.TimelineStore
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run

  setup do
    run_id = unique_run_id("memory")

    Repo.insert!(%Run{
      run_id: run_id,
      thread_id: "thread_#{run_id}",
      status: "running",
      owner_user_id: 55,
      latest_seq: 0
    })

    %{run_id: run_id}
  end

  test "upserts retention policy and applies defaults deterministically" do
    assert {:ok, policy} =
             TimelineStore.upsert_retention_policy("tool_event", %{
               raw_retention_class: "hot",
               chunk_retention_class: "durable",
               raw_ttl_seconds: 600,
               chunk_ttl_seconds: 86_400
             })

    assert policy.event_class == "tool_event"

    defaults = TimelineStore.apply_retention_defaults("tool_event", %{})
    assert defaults.event_class == "tool_event"
    assert defaults.raw_retention_class == "hot"
    assert defaults.chunk_retention_class == "durable"
    assert is_struct(defaults.raw_expires_at, DateTime)
    assert is_struct(defaults.chunk_expires_at, DateTime)
  end

  test "appends and lists raw events in stable sequence order", %{run_id: run_id} do
    assert {:ok, _event} =
             TimelineStore.append_raw_event(run_id, %{
               seq: 1,
               event_type: "run.delta",
               event_class: "conversation",
               payload: %{"delta" => "a"}
             })

    assert {:ok, _event} =
             TimelineStore.append_raw_event(run_id, %{
               seq: 2,
               event_type: "run.delta",
               event_class: "conversation",
               payload: %{"delta" => "b"}
             })

    events = TimelineStore.list_raw_events(run_id)
    assert Enum.map(events, &{&1.seq, &1.payload["delta"]}) == [{1, "a"}, {2, "b"}]

    assert 1 == TimelineStore.drop_raw_events_up_to(run_id, 1)
    remaining = TimelineStore.list_raw_events(run_id)
    assert Enum.map(remaining, & &1.seq) == [2]
  end

  test "raw event insertion enforces deterministic sequence uniqueness", %{run_id: run_id} do
    assert {:ok, _event} =
             TimelineStore.append_raw_event(run_id, %{
               seq: 5,
               event_type: "run.delta",
               event_class: "conversation",
               payload: %{"delta" => "x"}
             })

    assert {:error, changeset} =
             TimelineStore.append_raw_event(run_id, %{
               seq: 5,
               event_type: "run.delta",
               event_class: "conversation",
               payload: %{"delta" => "y"}
             })

    assert %{run_id: ["has already been taken"]} = errors_on(changeset)
  end

  test "inserts and lists L1/L2 chunks with time and level filters", %{run_id: run_id} do
    now = DateTime.utc_now() |> DateTime.truncate(:second)
    l1_start = DateTime.add(now, -600, :second)
    l1_end = DateTime.add(now, -1, :second)

    assert {:ok, _chunk} =
             TimelineStore.insert_chunk(run_id, %{
               chunk_id: "l1_chunk_1",
               level: 1,
               event_class: "conversation",
               retention_class: "durable",
               window_started_at: l1_start,
               window_ended_at: l1_end,
               source_event_start_seq: 1,
               source_event_end_seq: 30,
               summary: %{"text" => "l1"},
               token_count: 120
             })

    assert {:ok, _chunk} =
             TimelineStore.insert_chunk(run_id, %{
               chunk_id: "l2_chunk_1",
               level: 2,
               event_class: "conversation",
               retention_class: "archive",
               window_started_at: DateTime.add(now, -3_600, :second),
               window_ended_at: DateTime.add(now, -1_800, :second),
               source_chunk_ids: ["l1_chunk_1"],
               summary: %{"text" => "l2"},
               token_count: 40
             })

    all_chunks = TimelineStore.list_chunks(run_id)
    assert Enum.map(all_chunks, & &1.chunk_id) == ["l1_chunk_1", "l2_chunk_1"]

    l1_chunks = TimelineStore.list_chunks(run_id, level: 1)
    assert Enum.map(l1_chunks, & &1.chunk_id) == ["l1_chunk_1"]
  end

  test "chunk insertion rejects invalid levels", %{run_id: run_id} do
    now = DateTime.utc_now()

    assert {:error, changeset} =
             TimelineStore.insert_chunk(run_id, %{
               chunk_id: "bad_chunk",
               level: 9,
               event_class: "conversation",
               window_started_at: now,
               window_ended_at: now,
               summary: %{},
               token_count: 0
             })

    assert %{level: ["is invalid"]} = errors_on(changeset)
  end

  defp unique_run_id(prefix) do
    suffix = System.unique_integer([:positive])
    "#{prefix}_#{suffix}"
  end
end
