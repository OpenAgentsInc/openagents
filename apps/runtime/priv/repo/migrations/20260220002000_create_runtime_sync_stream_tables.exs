defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeSyncStreamTables do
  use Ecto.Migration

  def change do
    create table(:sync_stream_events, prefix: "runtime") do
      add :topic, :string, null: false
      add :watermark, :bigint, null: false
      add :doc_key, :string, null: false
      add :doc_version, :bigint, null: false
      add :payload, :map
      add :payload_hash, :binary

      timestamps(type: :utc_datetime_usec, updated_at: false)
    end

    create unique_index(:sync_stream_events, [:topic, :watermark], prefix: "runtime")

    create index(:sync_stream_events, [:topic, :doc_key, :watermark], prefix: "runtime")

    create index(:sync_stream_events, [:inserted_at], prefix: "runtime")

    create table(:sync_run_summaries, primary_key: false, prefix: "runtime") do
      add :doc_key, :string, primary_key: true
      add :doc_version, :bigint, null: false, default: 0
      add :payload, :map, null: false, default: %{}
      add :payload_hash, :binary

      timestamps(type: :utc_datetime_usec)
    end

    create index(:sync_run_summaries, [:updated_at], prefix: "runtime")

    create table(:sync_codex_worker_summaries, primary_key: false, prefix: "runtime") do
      add :doc_key, :string, primary_key: true
      add :doc_version, :bigint, null: false, default: 0
      add :payload, :map, null: false, default: %{}
      add :payload_hash, :binary

      timestamps(type: :utc_datetime_usec)
    end

    create index(:sync_codex_worker_summaries, [:updated_at], prefix: "runtime")
  end
end
