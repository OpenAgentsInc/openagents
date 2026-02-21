defmodule OpenAgentsRuntime.Memory.MemoryCompaction do
  @moduledoc """
  Auditable record for L1 compaction runs.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  schema "memory_compactions" do
    field :run_id, :string
    field :level, :integer
    field :trigger_type, :string
    field :status, :string
    field :input_event_start_seq, :integer
    field :input_event_end_seq, :integer
    field :input_event_count, :integer
    field :output_chunk_id, :string
    field :summary_hash, :string
    field :model_name, :string
    field :model_version, :string
    field :token_count_input, :integer
    field :token_count_output, :integer
    field :artifact_uri, :string
    field :metadata, :map
    field :error_message, :string
    field :started_at, :utc_datetime_usec
    field :completed_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec)
  end

  @required_fields ~w(run_id level trigger_type status input_event_count metadata started_at)a

  @type t :: %__MODULE__{
          run_id: String.t(),
          level: integer(),
          trigger_type: String.t(),
          status: String.t(),
          input_event_start_seq: integer() | nil,
          input_event_end_seq: integer() | nil,
          input_event_count: integer(),
          output_chunk_id: String.t() | nil,
          summary_hash: String.t() | nil,
          model_name: String.t() | nil,
          model_version: String.t() | nil,
          token_count_input: integer() | nil,
          token_count_output: integer() | nil,
          artifact_uri: String.t() | nil,
          metadata: map(),
          error_message: String.t() | nil,
          started_at: DateTime.t(),
          completed_at: DateTime.t() | nil
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(compaction, attrs) do
    compaction
    |> cast(
      attrs,
      @required_fields ++
        [
          :input_event_start_seq,
          :input_event_end_seq,
          :output_chunk_id,
          :summary_hash,
          :model_name,
          :model_version,
          :token_count_input,
          :token_count_output,
          :artifact_uri,
          :error_message,
          :completed_at
        ]
    )
    |> validate_required(@required_fields)
    |> validate_inclusion(:level, [1])
    |> validate_inclusion(:trigger_type, ["scheduled", "pressure"])
    |> validate_inclusion(:status, ["succeeded", "failed", "noop"])
    |> validate_number(:input_event_count, greater_than_or_equal_to: 0)
    |> foreign_key_constraint(:run_id, name: :memory_compactions_run_id_fkey)
  end
end
