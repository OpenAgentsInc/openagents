defmodule OpenAgentsRuntime.Sync.RunSummary do
  @moduledoc """
  Runtime-owned read model for run summary docs delivered by Khala.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  @primary_key {:doc_key, :string, autogenerate: false}

  schema "sync_run_summaries" do
    field :doc_version, :integer, default: 0
    field :payload, :map, default: %{}
    field :payload_hash, :binary

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(doc_key doc_version payload)a
  @optional_fields ~w(payload_hash)a

  @type t :: %__MODULE__{
          doc_key: String.t(),
          doc_version: non_neg_integer(),
          payload: map(),
          payload_hash: binary() | nil,
          inserted_at: DateTime.t(),
          updated_at: DateTime.t()
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(summary, attrs) do
    summary
    |> cast(attrs, @required_fields ++ @optional_fields)
    |> validate_required(@required_fields)
    |> validate_length(:doc_key, min: 1, max: 255)
    |> validate_number(:doc_version, greater_than_or_equal_to: 0)
  end
end
