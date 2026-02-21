defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeTimelineMemoryTables do
  use Ecto.Migration

  def change do
    create table(:timeline_events, prefix: "runtime") do
      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :seq, :bigint, null: false
      add :event_type, :string, null: false
      add :event_class, :string, null: false, default: "default"
      add :retention_class, :string, null: false, default: "hot"
      add :payload, :map, null: false, default: %{}
      add :occurred_at, :utc_datetime_usec
      add :expires_at, :utc_datetime_usec

      timestamps(type: :utc_datetime_usec, updated_at: false)
    end

    create constraint(:timeline_events, :timeline_events_retention_class_valid,
             check: "retention_class IN ('hot', 'durable', 'compact_only', 'archive')",
             prefix: "runtime"
           )

    create unique_index(:timeline_events, [:run_id, :seq], prefix: "runtime")
    create index(:timeline_events, [:run_id, :event_class, :seq], prefix: "runtime")
    create index(:timeline_events, [:retention_class, :expires_at], prefix: "runtime")

    create table(:memory_chunks, prefix: "runtime") do
      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :chunk_id, :string, null: false
      add :level, :integer, null: false
      add :retention_class, :string, null: false, default: "durable"
      add :event_class, :string, null: false, default: "default"
      add :window_started_at, :utc_datetime_usec, null: false
      add :window_ended_at, :utc_datetime_usec, null: false
      add :source_event_start_seq, :bigint
      add :source_event_end_seq, :bigint
      add :source_chunk_ids, {:array, :string}, null: false, default: []
      add :summary, :map, null: false, default: %{}
      add :token_count, :integer, null: false, default: 0
      add :storage_uri, :string
      add :expires_at, :utc_datetime_usec

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:memory_chunks, :memory_chunks_level_valid,
             check: "level IN (1, 2, 3)",
             prefix: "runtime"
           )

    create constraint(:memory_chunks, :memory_chunks_retention_class_valid,
             check: "retention_class IN ('hot', 'durable', 'compact_only', 'archive')",
             prefix: "runtime"
           )

    create unique_index(:memory_chunks, [:run_id, :chunk_id], prefix: "runtime")
    create index(:memory_chunks, [:run_id, :level, :window_started_at], prefix: "runtime")
    create index(:memory_chunks, [:retention_class, :expires_at], prefix: "runtime")

    create table(:memory_retention_policies, primary_key: false, prefix: "runtime") do
      add :event_class, :string, primary_key: true, null: false
      add :raw_retention_class, :string, null: false
      add :chunk_retention_class, :string, null: false
      add :raw_ttl_seconds, :integer
      add :chunk_ttl_seconds, :integer
      add :retain_forever, :boolean, null: false, default: false

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:memory_retention_policies, :memory_retention_policies_raw_class_valid,
             check: "raw_retention_class IN ('hot', 'durable', 'compact_only', 'archive')",
             prefix: "runtime"
           )

    create constraint(:memory_retention_policies, :memory_retention_policies_chunk_class_valid,
             check: "chunk_retention_class IN ('hot', 'durable', 'compact_only', 'archive')",
             prefix: "runtime"
           )
  end
end
