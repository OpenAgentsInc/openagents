defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeSpendAuthorizations do
  use Ecto.Migration

  def change do
    create table(:spend_authorizations, primary_key: false, prefix: "runtime") do
      add :authorization_id, :string, primary_key: true
      add :owner_user_id, :bigint
      add :owner_guest_scope, :string
      add :autopilot_id, :string
      add :thread_id, :string

      add :run_id,
          references(:runs,
            column: :run_id,
            type: :string,
            prefix: "runtime",
            on_delete: :nilify_all
          )

      add :mode, :string, null: false, default: "delegated_budget"
      add :max_total_sats, :bigint
      add :max_per_call_sats, :bigint
      add :max_per_day_sats, :bigint
      add :threshold_sats, :bigint
      add :spent_sats, :bigint, null: false, default: 0
      add :reserved_sats, :bigint, null: false, default: 0
      add :constraints, :map, null: false, default: %{}
      add :metadata, :map, null: false, default: %{}
      add :issued_at, :utc_datetime_usec, null: false
      add :expires_at, :utc_datetime_usec
      add :revoked_at, :utc_datetime_usec

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:spend_authorizations, :spend_authorizations_mode_valid,
             check:
               "mode IN ('interactive', 'delegated_budget', 'deny', 'delegated_budget_with_threshold')",
             prefix: "runtime"
           )

    create constraint(:spend_authorizations, :spend_authorizations_owner_binding_valid,
             check:
               "(owner_user_id IS NOT NULL AND owner_guest_scope IS NULL) OR (owner_user_id IS NULL AND owner_guest_scope IS NOT NULL)",
             prefix: "runtime"
           )

    create constraint(:spend_authorizations, :spend_authorizations_amounts_non_negative,
             check:
               "COALESCE(max_total_sats, 0) >= 0 AND COALESCE(max_per_call_sats, 0) >= 0 AND COALESCE(max_per_day_sats, 0) >= 0 AND COALESCE(threshold_sats, 0) >= 0 AND spent_sats >= 0 AND reserved_sats >= 0",
             prefix: "runtime"
           )

    create index(:spend_authorizations, [:owner_user_id, :inserted_at], prefix: "runtime")
    create index(:spend_authorizations, [:owner_guest_scope, :inserted_at], prefix: "runtime")

    create index(:spend_authorizations, [:run_id, :thread_id, :autopilot_id, :inserted_at],
             prefix: "runtime"
           )
  end
end
