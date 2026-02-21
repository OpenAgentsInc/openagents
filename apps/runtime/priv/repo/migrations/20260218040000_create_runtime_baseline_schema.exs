defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeBaselineSchema do
  use Ecto.Migration

  def up do
    execute("CREATE SCHEMA IF NOT EXISTS runtime")
    execute("CREATE EXTENSION IF NOT EXISTS pgcrypto")

    execute("CREATE SEQUENCE IF NOT EXISTS runtime.global_event_id_seq")

    create table(:runs, primary_key: false, prefix: "runtime") do
      add :run_id, :string, null: false, primary_key: true
      add :thread_id, :string, null: false
      add :status, :string, null: false, default: "created"
      add :owner_user_id, :bigint
      add :owner_guest_scope, :string
      add :latest_seq, :bigint, null: false, default: 0

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:runs, :runs_owner_present,
             check: "owner_user_id IS NOT NULL OR owner_guest_scope IS NOT NULL",
             prefix: "runtime"
           )

    create index(:runs, [:thread_id], prefix: "runtime")
    create index(:runs, [:status, :updated_at], prefix: "runtime")
  end

  def down do
    drop_if_exists table(:runs, prefix: "runtime")
    execute("DROP SEQUENCE IF EXISTS runtime.global_event_id_seq")
  end
end
