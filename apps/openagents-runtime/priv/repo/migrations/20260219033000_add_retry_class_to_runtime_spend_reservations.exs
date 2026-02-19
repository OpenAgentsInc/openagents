defmodule OpenAgentsRuntime.Repo.Migrations.AddRetryClassToRuntimeSpendReservations do
  use Ecto.Migration

  def up do
    alter table(:spend_reservations, prefix: "runtime") do
      add :retry_class, :string, null: false, default: "safe_retry"
    end

    execute(
      "UPDATE runtime.spend_reservations SET retry_class = 'safe_retry' WHERE retry_class IS NULL",
      "UPDATE runtime.spend_reservations SET retry_class = NULL"
    )

    create constraint(:spend_reservations, :spend_reservations_retry_class_valid,
             check: "retry_class IN ('safe_retry', 'dedupe_reconcile_required')",
             prefix: "runtime"
           )
  end

  def down do
    drop constraint(:spend_reservations, :spend_reservations_retry_class_valid, prefix: "runtime")

    alter table(:spend_reservations, prefix: "runtime") do
      remove :retry_class
    end
  end
end
