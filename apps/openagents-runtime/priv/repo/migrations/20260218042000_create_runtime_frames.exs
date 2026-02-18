defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeFrames do
  use Ecto.Migration

  def change do
    create table(:frames, prefix: "runtime") do
      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :frame_id, :string, null: false
      add :frame_type, :string, null: false
      add :payload, :map, null: false, default: %{}
      add :payload_hash, :string, null: false
      add :occurred_at, :utc_datetime_usec

      timestamps(type: :utc_datetime_usec, updated_at: false)
    end

    create constraint(:frames, :frames_frame_id_nonempty,
             check: "char_length(frame_id) > 0",
             prefix: "runtime"
           )

    create unique_index(:frames, [:run_id, :frame_id], prefix: "runtime")
    create index(:frames, [:run_id, :inserted_at], prefix: "runtime")
  end
end
