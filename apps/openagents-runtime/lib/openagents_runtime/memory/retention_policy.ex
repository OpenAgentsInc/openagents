defmodule OpenAgentsRuntime.Memory.RetentionPolicy do
  @moduledoc """
  Retention policy by event class for raw timeline events and compacted chunks.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @primary_key {:event_class, :string, autogenerate: false}
  @schema_prefix "runtime"
  schema "memory_retention_policies" do
    field :raw_retention_class, :string
    field :chunk_retention_class, :string
    field :raw_ttl_seconds, :integer
    field :chunk_ttl_seconds, :integer
    field :retain_forever, :boolean

    timestamps(type: :utc_datetime_usec)
  end

  @retention_classes ~w(hot durable compact_only archive)
  @required_fields ~w(event_class raw_retention_class chunk_retention_class retain_forever)a

  @type t :: %__MODULE__{
          event_class: String.t(),
          raw_retention_class: String.t(),
          chunk_retention_class: String.t(),
          raw_ttl_seconds: integer() | nil,
          chunk_ttl_seconds: integer() | nil,
          retain_forever: boolean()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(policy, attrs) do
    policy
    |> cast(attrs, @required_fields ++ [:raw_ttl_seconds, :chunk_ttl_seconds])
    |> validate_required(@required_fields)
    |> validate_length(:event_class, min: 1)
    |> validate_inclusion(:raw_retention_class, @retention_classes)
    |> validate_inclusion(:chunk_retention_class, @retention_classes)
    |> validate_number(:raw_ttl_seconds, greater_than: 0)
    |> validate_number(:chunk_ttl_seconds, greater_than: 0)
  end
end
