defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeSpendReservations do
  use Ecto.Migration

  def change do
    create table(:spend_reservations, prefix: "runtime") do
      add :authorization_id,
          references(:spend_authorizations,
            column: :authorization_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :delete_all
          ),
          null: false

      add :tool_call_id, :string, null: false
      add :amount_sats, :bigint, null: false
      add :state, :string, null: false, default: "reserved"
      add :provider_correlation_id, :string
      add :provider_idempotency_key, :string
      add :failure_reason, :string
      add :metadata, :map, null: false, default: %{}
      add :reserved_at, :utc_datetime_usec, null: false
      add :committed_at, :utc_datetime_usec
      add :released_at, :utc_datetime_usec
      add :reconciled_at, :utc_datetime_usec

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:spend_reservations, :spend_reservations_amount_positive,
             check: "amount_sats > 0",
             prefix: "runtime"
           )

    create constraint(:spend_reservations, :spend_reservations_state_valid,
             check: "state IN ('reserved', 'committed', 'released', 'reconcile_required')",
             prefix: "runtime"
           )

    create unique_index(:spend_reservations, [:authorization_id, :run_id, :tool_call_id],
             prefix: "runtime"
           )

    create index(:spend_reservations, [:authorization_id, :state, :reserved_at],
             prefix: "runtime"
           )

    create index(:spend_reservations, [:run_id, :state, :inserted_at], prefix: "runtime")
  end
end
