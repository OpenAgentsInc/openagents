defmodule OpenAgentsRuntime.Sync.TopicPolicyTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Sync.TopicPolicy

  @run_topic "runtime.run_summaries"
  @worker_events_topic "runtime.codex_worker_events"

  test "default policy exposes retention and snapshot metadata for summary topics" do
    policies = TopicPolicy.topic_policies()

    assert TopicPolicy.retention_seconds(@run_topic, policies, 86_400) == 604_800
    assert TopicPolicy.topic_class(@run_topic, policies) == "durable_summary"

    assert TopicPolicy.compaction_mode(@run_topic, policies) ==
             "tail_prune_with_snapshot_rehydrate"

    snapshot = TopicPolicy.snapshot_metadata(@run_topic, policies)
    assert snapshot["topic"] == @run_topic
    assert snapshot["format"] == "openagents.sync.snapshot.v1"
    assert snapshot["schema_version"] == 1
    assert snapshot["source_table"] == "runtime.sync_run_summaries"
  end

  test "event topics default to no snapshot metadata" do
    policies = TopicPolicy.topic_policies()

    assert TopicPolicy.topic_class(@worker_events_topic, policies) == "high_churn_events"
    assert TopicPolicy.snapshot_metadata(@worker_events_topic, policies) == nil
  end

  test "configured policy overrides retention window and snapshot settings" do
    policies =
      TopicPolicy.topic_policies(%{
        @worker_events_topic => %{
          retention_seconds: 7_200,
          topic_class: "event_hot",
          compaction_mode: "tail_prune_with_snapshot_rehydrate",
          snapshot: %{
            enabled: true,
            schema_version: 2,
            source_table: "runtime.sync_worker_event_snapshots"
          }
        }
      })

    assert TopicPolicy.retention_seconds(@worker_events_topic, policies, 86_400) == 7_200
    assert TopicPolicy.topic_class(@worker_events_topic, policies) == "event_hot"

    assert TopicPolicy.compaction_mode(@worker_events_topic, policies) ==
             "tail_prune_with_snapshot_rehydrate"

    snapshot = TopicPolicy.snapshot_metadata(@worker_events_topic, policies)
    assert snapshot["schema_version"] == 2
    assert snapshot["source_table"] == "runtime.sync_worker_event_snapshots"
  end
end
