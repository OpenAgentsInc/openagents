defmodule OpenAgentsRuntime.Repo.Migrations.AddRuntimeRunRecoveryFields do
  use Ecto.Migration

  def change do
    alter table(:runs, prefix: "runtime") do
      add :recovery_attempt_count, :integer, null: false, default: 0
      add :last_recovery_at, :utc_datetime_usec
    end

    create index(:runs, [:recovery_attempt_count, :last_recovery_at], prefix: "runtime")
  end
end
