defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeDsPointerAudits do
  use Ecto.Migration

  def change do
    create table(:ds_pointer_audits, prefix: "runtime") do
      add :signature_id, :string, null: false
      add :action, :string, null: false
      add :actor, :string
      add :reason, :text
      add :metadata, :map, null: false, default: %{}
      add :before_pointer, :map
      add :after_pointer, :map
      add :target_audit_id, :bigint

      timestamps(type: :utc_datetime_usec, updated_at: false)
    end

    create constraint(:ds_pointer_audits, :ds_pointer_audits_action_valid,
             check: "action IN ('promote', 'rollback', 'set_canary', 'clear_canary')",
             prefix: "runtime"
           )

    create index(:ds_pointer_audits, [:signature_id, :inserted_at], prefix: "runtime")
    create index(:ds_pointer_audits, [:target_audit_id], prefix: "runtime")
  end
end
