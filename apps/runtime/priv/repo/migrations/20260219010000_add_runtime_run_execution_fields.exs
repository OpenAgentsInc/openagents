defmodule OpenAgentsRuntime.Repo.Migrations.AddRuntimeRunExecutionFields do
  use Ecto.Migration

  def change do
    alter table(:runs, prefix: "runtime") do
      add :last_processed_frame_id, :bigint, null: false, default: 0
      add :terminal_reason_class, :string
      add :terminal_reason, :string
      add :terminal_at, :utc_datetime_usec
    end

    create index(:runs, [:status, :terminal_at], prefix: "runtime")
  end
end
