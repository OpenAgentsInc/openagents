defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeCodexWorkerTables do
  use Ecto.Migration

  def change do
    create table(:codex_workers, primary_key: false, prefix: "runtime") do
      add :worker_id, :string, primary_key: true
      add :owner_user_id, :integer
      add :owner_guest_scope, :string
      add :workspace_ref, :string
      add :codex_home_ref, :string
      add :adapter, :string, null: false, default: "in_memory"
      add :status, :string, null: false, default: "running"
      add :latest_seq, :bigint, null: false, default: 0
      add :metadata, :map, null: false, default: %{}
      add :started_at, :utc_datetime_usec, null: false
      add :stopped_at, :utc_datetime_usec
      add :last_heartbeat_at, :utc_datetime_usec

      timestamps(type: :utc_datetime_usec)
    end

    create index(:codex_workers, [:owner_user_id, :status], prefix: "runtime")
    create index(:codex_workers, [:owner_guest_scope, :status], prefix: "runtime")

    create table(:codex_worker_events, primary_key: false, prefix: "runtime") do
      add :worker_id,
          references(:codex_workers,
            column: :worker_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :seq, :bigint, null: false
      add :event_type, :string, null: false
      add :payload, :map, null: false, default: %{}

      timestamps(type: :utc_datetime_usec, updated_at: false)
    end

    create unique_index(:codex_worker_events, [:worker_id, :seq], prefix: "runtime")
    create index(:codex_worker_events, [:worker_id, :inserted_at], prefix: "runtime")
  end
end
