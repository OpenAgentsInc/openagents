defmodule OpenAgentsRuntime.Sync.TopicSequence do
  @moduledoc """
  Monotonic per-topic watermark sequence state for Khala replay allocation.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @primary_key {:topic, :string, autogenerate: false}

  schema "sync_topic_sequences" do
    field :next_watermark, :integer, default: 0

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(topic next_watermark)a

  @type t :: %__MODULE__{
          topic: String.t(),
          next_watermark: non_neg_integer(),
          inserted_at: DateTime.t(),
          updated_at: DateTime.t()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(topic_sequence, attrs) do
    topic_sequence
    |> cast(attrs, @required_fields)
    |> validate_required(@required_fields)
    |> validate_length(:topic, min: 1, max: 160)
    |> validate_number(:next_watermark, greater_than_or_equal_to: 0)
  end
end
