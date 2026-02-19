defmodule OpenAgentsRuntime.Codex.WorkerEvent do
  @moduledoc """
  Durable event log row for Codex worker sessions.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @primary_key false

  schema "codex_worker_events" do
    field :worker_id, :string
    field :seq, :integer
    field :event_type, :string
    field :payload, :map

    timestamps(type: :utc_datetime_usec, updated_at: false)
  end

  @required_fields ~w(worker_id seq event_type payload)a

  @type t :: %__MODULE__{
          worker_id: String.t(),
          seq: pos_integer(),
          event_type: String.t(),
          payload: map()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(event, attrs) do
    event
    |> cast(attrs, @required_fields)
    |> validate_required(@required_fields)
    |> validate_number(:seq, greater_than: 0)
    |> validate_length(:event_type, min: 1, max: 120)
    |> foreign_key_constraint(:worker_id, name: :codex_worker_events_worker_id_fkey)
    |> unique_constraint([:worker_id, :seq], name: :codex_worker_events_worker_id_seq_index)
  end
end
