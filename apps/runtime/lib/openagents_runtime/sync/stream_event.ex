defmodule OpenAgentsRuntime.Sync.StreamEvent do
  @moduledoc """
  Durable ordered event journal used for Khala replay by topic watermark.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"

  schema "sync_stream_events" do
    field :topic, :string
    field :watermark, :integer
    field :doc_key, :string
    field :doc_version, :integer
    field :payload, :map
    field :payload_hash, :binary

    timestamps(type: :utc_datetime_usec, updated_at: false)
  end

  @required_fields ~w(topic watermark doc_key doc_version)a
  @optional_fields ~w(payload payload_hash)a

  @type t :: %__MODULE__{
          topic: String.t(),
          watermark: non_neg_integer(),
          doc_key: String.t(),
          doc_version: non_neg_integer(),
          payload: map() | nil,
          payload_hash: binary() | nil,
          inserted_at: DateTime.t()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(stream_event, attrs) do
    stream_event
    |> cast(attrs, @required_fields ++ @optional_fields)
    |> validate_required(@required_fields)
    |> validate_length(:topic, min: 1, max: 160)
    |> validate_length(:doc_key, min: 1, max: 255)
    |> validate_number(:watermark, greater_than_or_equal_to: 0)
    |> validate_number(:doc_version, greater_than_or_equal_to: 0)
    |> unique_constraint([:topic, :watermark], name: :sync_stream_events_topic_watermark_index)
  end
end
