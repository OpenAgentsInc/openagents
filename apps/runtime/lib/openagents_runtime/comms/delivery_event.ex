defmodule OpenAgentsRuntime.Comms.DeliveryEvent do
  @moduledoc """
  Canonical runtime record for normalized comms delivery webhooks.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"

  @delivery_states ~w(delivered bounced complained unsubscribed)

  schema "comms_delivery_events" do
    field :event_id, :string
    field :provider, :string
    field :delivery_state, :string
    field :message_id, :string
    field :integration_id, :string
    field :recipient, :string
    field :occurred_at, :utc_datetime_usec
    field :reason, :string
    field :payload, :map
    field :payload_hash, :string

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(event_id provider delivery_state payload payload_hash)a

  @type t :: %__MODULE__{
          event_id: String.t(),
          provider: String.t(),
          delivery_state: String.t(),
          message_id: String.t() | nil,
          integration_id: String.t() | nil,
          recipient: String.t() | nil,
          occurred_at: DateTime.t() | nil,
          reason: String.t() | nil,
          payload: map(),
          payload_hash: String.t()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(event, attrs) do
    event
    |> cast(
      attrs,
      @required_fields ++ ~w(message_id integration_id recipient occurred_at reason)a
    )
    |> validate_required(@required_fields)
    |> validate_length(:event_id, min: 2, max: 256)
    |> validate_length(:provider, min: 2, max: 64)
    |> validate_inclusion(:delivery_state, @delivery_states)
    |> unique_constraint(:event_id)
  end
end
