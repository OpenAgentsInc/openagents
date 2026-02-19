defmodule OpenAgentsRuntime.Memory.MemoryChunk do
  @moduledoc """
  Compacted memory chunk for L1/L2/L3 timeline rollups.
  """

  use Ecto.Schema

  import Ecto.Changeset

  @schema_prefix "runtime"
  schema "memory_chunks" do
    field :run_id, :string
    field :chunk_id, :string
    field :level, :integer
    field :retention_class, :string
    field :event_class, :string
    field :window_started_at, :utc_datetime_usec
    field :window_ended_at, :utc_datetime_usec
    field :source_event_start_seq, :integer
    field :source_event_end_seq, :integer
    field :source_chunk_ids, {:array, :string}
    field :summary, :map
    field :token_count, :integer
    field :storage_uri, :string
    field :expires_at, :utc_datetime_usec

    timestamps(type: :utc_datetime_usec)
  end

  @retention_classes ~w(hot durable compact_only archive)
  @required_fields ~w(
    run_id
    chunk_id
    level
    retention_class
    event_class
    window_started_at
    window_ended_at
    summary
    token_count
  )a

  @type t :: %__MODULE__{
          run_id: String.t(),
          chunk_id: String.t(),
          level: 1 | 2 | 3,
          retention_class: String.t(),
          event_class: String.t(),
          window_started_at: DateTime.t(),
          window_ended_at: DateTime.t(),
          source_event_start_seq: integer() | nil,
          source_event_end_seq: integer() | nil,
          source_chunk_ids: [String.t()],
          summary: map(),
          token_count: non_neg_integer(),
          storage_uri: String.t() | nil,
          expires_at: DateTime.t() | nil
        }

  @spec changeset(t(), map()) :: Ecto.Changeset.t()
  def changeset(chunk, attrs) do
    changeset =
      chunk
      |> cast(
        attrs,
        @required_fields ++
          [
            :source_event_start_seq,
            :source_event_end_seq,
            :source_chunk_ids,
            :storage_uri,
            :expires_at
          ]
      )
      |> validate_required(@required_fields)
      |> validate_inclusion(:level, [1, 2, 3])
      |> validate_inclusion(:retention_class, @retention_classes)
      |> validate_number(:token_count, greater_than_or_equal_to: 0)

    started_at = get_field(changeset, :window_started_at)

    changeset
    |> validate_change(:window_ended_at, fn :window_ended_at, value ->
      if is_struct(started_at, DateTime) and DateTime.compare(value, started_at) == :lt do
        [window_ended_at: "must be on or after window_started_at"]
      else
        []
      end
    end)
    |> foreign_key_constraint(:run_id, name: :memory_chunks_run_id_fkey)
    |> unique_constraint([:run_id, :chunk_id], name: :memory_chunks_run_id_chunk_id_index)
  end
end
