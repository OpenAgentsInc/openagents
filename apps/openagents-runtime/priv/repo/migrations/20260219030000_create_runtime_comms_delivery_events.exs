defmodule OpenAgentsRuntime.Repo.Migrations.CreateRuntimeCommsDeliveryEvents do
  use Ecto.Migration

  def change do
    create table(:comms_delivery_events, prefix: "runtime") do
      add :event_id, :string, null: false
      add :provider, :string, null: false
      add :delivery_state, :string, null: false
      add :message_id, :string
      add :integration_id, :string
      add :recipient, :string
      add :occurred_at, :utc_datetime_usec
      add :reason, :string
      add :payload, :map, null: false, default: %{}
      add :payload_hash, :string, null: false

      timestamps(type: :utc_datetime_usec)
    end

    create constraint(:comms_delivery_events, :comms_delivery_events_state_allowed,
             check: "delivery_state IN ('delivered','bounced','complained','unsubscribed')",
             prefix: "runtime"
           )

    create unique_index(:comms_delivery_events, [:event_id], prefix: "runtime")
    create index(:comms_delivery_events, [:provider, :integration_id], prefix: "runtime")
    create index(:comms_delivery_events, [:message_id], prefix: "runtime")
    create index(:comms_delivery_events, [:inserted_at], prefix: "runtime")
  end
end
