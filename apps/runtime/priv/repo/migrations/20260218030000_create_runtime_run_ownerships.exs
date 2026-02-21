defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeRunOwnerships do
  use Ecto.Migration

  def up do
    execute("CREATE SCHEMA IF NOT EXISTS runtime")

    create table(:run_ownerships, primary_key: false, prefix: "runtime") do
      add :run_id, :string, null: false, primary_key: true
      add :thread_id, :string, null: false, primary_key: true
      add :user_id, :bigint
      add :guest_scope, :string

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:run_ownerships, :run_ownerships_owner_present,
             check: "user_id IS NOT NULL OR guest_scope IS NOT NULL",
             prefix: "runtime"
           )

    create unique_index(:run_ownerships, [:run_id, :thread_id], prefix: "runtime")
  end

  def down do
    drop_if_exists table(:run_ownerships, prefix: "runtime")
  end
end
