defmodule OpenAgentsRuntime.Repo.Migrations.AddRuntimeCodexWorkerEventsSyncTopic do
  use Ecto.Migration

  @topic "runtime.codex_worker_events"

  def up do
    execute("""
    INSERT INTO runtime.sync_topic_sequences (topic, next_watermark, inserted_at, updated_at)
    VALUES ('#{@topic}', 0, now(), now())
    ON CONFLICT (topic) DO NOTHING
    """)
  end

  def down do
    execute("DELETE FROM runtime.sync_topic_sequences WHERE topic = '#{@topic}'")
  end
end
