defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeRunLeases do
  use Ecto.Migration

  def change do
    create table(:run_leases, primary_key: false, prefix: "runtime") do
      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          primary_key: true,
          null: false

      add :lease_owner, :string, null: false
      add :lease_expires_at, :utc_datetime_usec, null: false
      add :last_progress_seq, :bigint, null: false, default: 0
      add :heartbeat_at, :utc_datetime_usec, null: false

      timestamps(type: :utc_datetime_usec)
    end

    create index(:run_leases, [:lease_owner], prefix: "runtime")
    create index(:run_leases, [:lease_expires_at], prefix: "runtime")
  end
end
