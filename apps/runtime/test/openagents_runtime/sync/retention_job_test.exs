defmodule OpenAgentsRuntime.Sync.RetentionJobTest do
  use OpenAgentsRuntime.DataCase, async: false

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.RetentionJob
  alias OpenAgentsRuntime.Sync.StreamEvent

  @cycle_event [:openagents_runtime, :sync, :retention, :cycle]
  @topic_event [:openagents_runtime, :sync, :retention, :topic]

  @run_topic "runtime.run_summaries"
  @worker_topic "runtime.codex_worker_summaries"
  @worker_events_topic "runtime.codex_worker_events"

  test "run_once prunes expired rows and emits retention telemetry" do
    now = ~U[2026-02-20 12:00:00.000000Z]

    insert_stream_event(@run_topic, 1, DateTime.add(now, -7_200, :second))
    insert_stream_event(@run_topic, 2, DateTime.add(now, -60, :second))

    {cycle_ref, topic_ref} = attach_telemetry(self())

    on_exit(fn ->
      :telemetry.detach(cycle_ref)
      :telemetry.detach(topic_ref)
    end)

    summary = RetentionJob.run_once(now: now, horizon_seconds: 3_600, batch_size: 10)

    assert summary.deleted == 1
    assert summary.oldest_retained[@run_topic] == 2
    assert summary.oldest_retained[@worker_topic] == nil
    assert summary.oldest_retained[@worker_events_topic] == nil

    assert_receive {:retention_cycle, measurements, metadata}
    assert measurements.deleted == 1
    assert metadata.component == "sync_retention_job"

    assert_receive {:retention_topic, topic_measurements, topic_metadata}

    assert topic_metadata.event_type in [
             @run_topic,
             @worker_topic,
             @worker_events_topic,
             "runtime.notifications"
           ]

    assert topic_measurements.count == 1

    remaining =
      from(event in StreamEvent,
        where: event.topic == ^@run_topic,
        select: event.watermark
      )
      |> Repo.all()

    assert remaining == [2]
  end

  test "run_once reports nil oldest watermark for empty topic streams" do
    now = ~U[2026-02-20 12:00:00.000000Z]

    summary = RetentionJob.run_once(now: now, horizon_seconds: 3_600, batch_size: 10)

    assert summary.deleted == 0
    assert summary.oldest_retained[@run_topic] == nil
    assert summary.oldest_retained[@worker_topic] == nil
    assert summary.oldest_retained[@worker_events_topic] == nil
    assert summary.oldest_retained["runtime.notifications"] == nil
  end

  defp insert_stream_event(topic, watermark, inserted_at) do
    Repo.insert_all(
      StreamEvent,
      [
        %{
          topic: topic,
          watermark: watermark,
          doc_key: "#{topic}:#{watermark}",
          doc_version: watermark,
          payload: %{"watermark" => watermark},
          payload_hash: :crypto.hash(:sha256, Integer.to_string(watermark)),
          inserted_at: inserted_at
        }
      ]
    )
  end

  defp attach_telemetry(test_pid) do
    cycle_ref = "sync-retention-cycle-#{System.unique_integer([:positive])}"
    topic_ref = "sync-retention-topic-#{System.unique_integer([:positive])}"

    :ok =
      :telemetry.attach(
        cycle_ref,
        @cycle_event,
        fn _event_name, measurements, metadata, pid ->
          send(pid, {:retention_cycle, measurements, metadata})
        end,
        test_pid
      )

    :ok =
      :telemetry.attach(
        topic_ref,
        @topic_event,
        fn _event_name, measurements, metadata, pid ->
          send(pid, {:retention_topic, measurements, metadata})
        end,
        test_pid
      )

    {cycle_ref, topic_ref}
  end
end
