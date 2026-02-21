defmodule OpenAgentsRuntime.Memory.MemoryRollup do
  @moduledoc """
  Auditable rollup record for L2/L3 memory chunk creation.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  schema "memory_rollups" do
    field :run_id, :string
    field :source_level, :integer
    field :target_level, :integer
    field :source_chunk_ids, {:array, :string}
    field :output_chunk_id, :string
    field :summary_hash, :string
    field :status, :string
    field :metadata, :map
    field :started_at, :utc_datetime_usec
    field :completed_at, :utc_datetime_usec
    field :error_message, :string

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(run_id source_level target_level source_chunk_ids output_chunk_id status metadata started_at)a

  @type t :: %__MODULE__{
          run_id: String.t(),
          source_level: integer(),
          target_level: integer(),
          source_chunk_ids: [String.t()],
          output_chunk_id: String.t(),
          summary_hash: String.t() | nil,
          status: String.t(),
          metadata: map(),
          started_at: DateTime.t(),
          completed_at: DateTime.t() | nil,
          error_message: String.t() | nil
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(rollup, attrs) do
    rollup
    |> cast(attrs, @required_fields ++ [:summary_hash, :completed_at, :error_message])
    |> validate_required(@required_fields)
    |> validate_inclusion(:source_level, [1, 2])
    |> validate_inclusion(:target_level, [2, 3])
    |> validate_inclusion(:status, ["succeeded", "failed", "noop"])
    |> validate_length(:output_chunk_id, min: 1)
    |> foreign_key_constraint(:run_id, name: :memory_rollups_run_id_fkey)
    |> unique_constraint([:run_id, :output_chunk_id],
      name: :memory_rollups_run_id_output_chunk_id_index
    )
  end
end
