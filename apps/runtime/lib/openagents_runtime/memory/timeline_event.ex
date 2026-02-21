defmodule OpenAgentsRuntime.Memory.TimelineEvent do
  @moduledoc """
  Raw runtime timeline event persisted for short-horizon replay and compaction.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  schema "timeline_events" do
    field :run_id, :string
    field :seq, :integer
    field :event_type, :string
    field :event_class, :string
    field :retention_class, :string
    field :payload, :map
    field :occurred_at, :utc_datetime_usec
    field :expires_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec, updated_at: false)
  end

  @retention_classes ~w(hot durable compact_only archive)
  @required_fields ~w(run_id seq event_type event_class retention_class payload)a

  @type t :: %__MODULE__{
          run_id: String.t(),
          seq: pos_integer(),
          event_type: String.t(),
          event_class: String.t(),
          retention_class: String.t(),
          payload: map(),
          occurred_at: DateTime.t() | nil,
          expires_at: DateTime.t() | nil
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(event, attrs) do
    event
    |> cast(attrs, @required_fields ++ [:occurred_at, :expires_at])
    |> validate_required(@required_fields)
    |> validate_number(:seq, greater_than: 0)
    |> validate_length(:event_type, min: 1)
    |> validate_length(:event_class, min: 1)
    |> validate_inclusion(:retention_class, @retention_classes)
    |> foreign_key_constraint(:run_id, name: :timeline_events_run_id_fkey)
    |> unique_constraint([:run_id, :seq], name: :timeline_events_run_id_seq_index)
  end
end
