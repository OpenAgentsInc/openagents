defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeSyncTopicSequences do
  use Ecto.Migration

  @topics [
    "runtime.run_summaries",
    "runtime.codex_worker_summaries",
    "runtime.notifications"
  ]

  def up do
    create table(:sync_topic_sequences, primary_key: false, prefix: "runtime") do
      add :topic, :string, primary_key: true
      add :next_watermark, :bigint, null: false, default: 0

      timestamps(type: :utc_datetime_usec)
    end

    execute("""
    INSERT INTO runtime.sync_topic_sequences (topic, next_watermark, inserted_at, updated_at)
    VALUES #{Enum.map_join(@topics, ",", fn topic -> "('#{topic}', 0, now(), now())" end)}
    ON CONFLICT (topic) DO NOTHING
    """)
  end

  def down do
    drop table(:sync_topic_sequences, prefix: "runtime")
  end
end
