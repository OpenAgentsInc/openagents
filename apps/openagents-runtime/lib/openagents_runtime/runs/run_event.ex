defmodule OpenAgentsRuntime.Runs.RunEvent do
  @moduledoc """
  Durable event log row for runtime runs.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  schema "run_events" do
    field :run_id, :string
    field :seq, :integer
    field :event_type, :string
    field :payload, :map

    timestamps(type: :utc_datetime_usec, updated_at: false)
  end

  @required_fields ~w(run_id seq event_type payload)a

  @type t :: %__MODULE__{
          run_id: String.t(),
          seq: integer(),
          event_type: String.t(),
          payload: map()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(event, attrs) do
    event
    |> cast(attrs, @required_fields)
    |> validate_required(@required_fields)
    |> validate_number(:seq, greater_than: 0)
  end
end
