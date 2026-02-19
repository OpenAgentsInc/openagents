defmodule OpenAgentsRuntime.Memory.TimelineStore do
  @moduledoc """
  Timeline and memory chunk persistence boundary with retention policies.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Memory.MemoryChunk
  alias OpenAgentsRuntime.Memory.MemoryCompaction
  alias OpenAgentsRuntime.Memory.RetentionPolicy
  alias OpenAgentsRuntime.Memory.TimelineEvent
  alias OpenAgentsRuntime.Repo

  @retention_classes ~w(hot durable compact_only archive)

  @spec build_pointer(String.t(), non_neg_integer()) :: String.t()
  def build_pointer(run_id, seq) when is_binary(run_id) and is_integer(seq) and seq >= 0 do
    "timeline:" <> run_id <> ":" <> Integer.to_string(seq)
  end

  @spec append_raw_event(String.t(), map()) :: {:ok, TimelineEvent.t()} | {:error, term()}
  def append_raw_event(run_id, attrs) when is_binary(run_id) and is_map(attrs) do
    retention_class =
      normalize_retention_class(attrs[:retention_class] || attrs["retention_class"])

    event_class = attrs[:event_class] || attrs["event_class"] || "default"
    occurred_at = attrs[:occurred_at] || attrs["occurred_at"] || DateTime.utc_now()

    expires_at =
      attrs[:expires_at] || attrs["expires_at"] ||
        expires_at_for(event_class, retention_class, occurred_at, :raw)

    changeset =
      TimelineEvent.changeset(%TimelineEvent{}, %{
        run_id: run_id,
        seq: attrs[:seq] || attrs["seq"],
        event_type: attrs[:event_type] || attrs["event_type"],
        event_class: event_class,
        retention_class: retention_class,
        payload: attrs[:payload] || attrs["payload"] || %{},
        occurred_at: occurred_at,
        expires_at: expires_at
      })

    Repo.insert(changeset)
  end

  @spec list_raw_events(String.t(), keyword()) :: [TimelineEvent.t()]
  def list_raw_events(run_id, opts \\ []) when is_binary(run_id) do
    since_seq = Keyword.get(opts, :since_seq, 0)
    upto_seq = Keyword.get(opts, :upto_seq)
    limit = Keyword.get(opts, :limit, 200)
    event_class = Keyword.get(opts, :event_class)

    query =
      from(event in TimelineEvent,
        where: event.run_id == ^run_id and event.seq > ^since_seq,
        order_by: [asc: event.seq],
        limit: ^limit
      )

    query =
      if is_binary(event_class) do
        from(event in query, where: event.event_class == ^event_class)
      else
        query
      end

    query =
      if is_integer(upto_seq) and upto_seq > 0 do
        from(event in query, where: event.seq <= ^upto_seq)
      else
        query
      end

    Repo.all(query)
  end

  @spec drop_raw_events_up_to(String.t(), non_neg_integer()) :: non_neg_integer()
  def drop_raw_events_up_to(run_id, max_seq)
      when is_binary(run_id) and is_integer(max_seq) and max_seq >= 0 do
    query =
      from(event in TimelineEvent,
        where: event.run_id == ^run_id and event.seq <= ^max_seq
      )

    {count, _} = Repo.delete_all(query)
    count
  end

  @spec insert_chunk(String.t(), map()) :: {:ok, MemoryChunk.t()} | {:error, term()}
  def insert_chunk(run_id, attrs) when is_binary(run_id) and is_map(attrs) do
    retention_class =
      normalize_retention_class(attrs[:retention_class] || attrs["retention_class"] || "durable")

    event_class = attrs[:event_class] || attrs["event_class"] || "default"

    window_started_at =
      attrs[:window_started_at] || attrs["window_started_at"] || DateTime.utc_now()

    window_ended_at = attrs[:window_ended_at] || attrs["window_ended_at"] || window_started_at

    expires_at =
      attrs[:expires_at] || attrs["expires_at"] ||
        expires_at_for(event_class, retention_class, window_ended_at, :chunk)

    changeset =
      MemoryChunk.changeset(%MemoryChunk{}, %{
        run_id: run_id,
        chunk_id: attrs[:chunk_id] || attrs["chunk_id"],
        level: attrs[:level] || attrs["level"],
        retention_class: retention_class,
        event_class: event_class,
        window_started_at: window_started_at,
        window_ended_at: window_ended_at,
        source_event_start_seq: attrs[:source_event_start_seq] || attrs["source_event_start_seq"],
        source_event_end_seq: attrs[:source_event_end_seq] || attrs["source_event_end_seq"],
        source_chunk_ids: attrs[:source_chunk_ids] || attrs["source_chunk_ids"] || [],
        summary: attrs[:summary] || attrs["summary"] || %{},
        token_count: attrs[:token_count] || attrs["token_count"] || 0,
        storage_uri: attrs[:storage_uri] || attrs["storage_uri"],
        expires_at: expires_at
      })

    Repo.insert(changeset)
  end

  @spec list_chunks(String.t(), keyword()) :: [MemoryChunk.t()]
  def list_chunks(run_id, opts \\ []) when is_binary(run_id) do
    level = Keyword.get(opts, :level)
    limit = Keyword.get(opts, :limit, 200)
    from_time = Keyword.get(opts, :from_time)

    query =
      from(chunk in MemoryChunk,
        where: chunk.run_id == ^run_id,
        order_by: [desc: chunk.window_started_at, desc: chunk.id],
        limit: ^limit
      )

    query =
      if is_integer(level), do: from(chunk in query, where: chunk.level == ^level), else: query

    query =
      if is_struct(from_time, DateTime) do
        from(chunk in query, where: chunk.window_ended_at >= ^from_time)
      else
        query
      end

    Repo.all(query)
  end

  @spec upsert_retention_policy(String.t(), map()) ::
          {:ok, RetentionPolicy.t()} | {:error, term()}
  def upsert_retention_policy(event_class, attrs) when is_binary(event_class) and is_map(attrs) do
    changeset =
      RetentionPolicy.changeset(%RetentionPolicy{}, %{
        event_class: event_class,
        raw_retention_class: attrs[:raw_retention_class] || attrs["raw_retention_class"] || "hot",
        chunk_retention_class:
          attrs[:chunk_retention_class] || attrs["chunk_retention_class"] || "durable",
        raw_ttl_seconds: attrs[:raw_ttl_seconds] || attrs["raw_ttl_seconds"],
        chunk_ttl_seconds: attrs[:chunk_ttl_seconds] || attrs["chunk_ttl_seconds"],
        retain_forever: attrs[:retain_forever] || attrs["retain_forever"] || false
      })

    Repo.insert(
      changeset,
      on_conflict: [
        set: [
          raw_retention_class: Ecto.Changeset.get_field(changeset, :raw_retention_class),
          chunk_retention_class: Ecto.Changeset.get_field(changeset, :chunk_retention_class),
          raw_ttl_seconds: Ecto.Changeset.get_field(changeset, :raw_ttl_seconds),
          chunk_ttl_seconds: Ecto.Changeset.get_field(changeset, :chunk_ttl_seconds),
          retain_forever: Ecto.Changeset.get_field(changeset, :retain_forever),
          updated_at: DateTime.utc_now()
        ]
      ],
      conflict_target: [:event_class]
    )
  end

  @spec get_retention_policy(String.t()) :: RetentionPolicy.t() | nil
  def get_retention_policy(event_class) when is_binary(event_class) do
    Repo.get(RetentionPolicy, event_class)
  end

  @spec insert_compaction(map()) :: {:ok, MemoryCompaction.t()} | {:error, term()}
  def insert_compaction(attrs) when is_map(attrs) do
    changeset = MemoryCompaction.changeset(%MemoryCompaction{}, attrs)
    Repo.insert(changeset)
  end

  @spec list_compactions(String.t(), keyword()) :: [MemoryCompaction.t()]
  def list_compactions(run_id, opts \\ []) when is_binary(run_id) do
    limit = Keyword.get(opts, :limit, 50)

    query =
      from(compaction in MemoryCompaction,
        where: compaction.run_id == ^run_id,
        order_by: [desc: compaction.inserted_at],
        limit: ^limit
      )

    Repo.all(query)
  end

  @spec apply_retention_defaults(String.t(), map()) :: map()
  def apply_retention_defaults(event_class, attrs)
      when is_binary(event_class) and is_map(attrs) do
    policy = get_retention_policy(event_class)
    now = DateTime.utc_now()

    raw_retention_class =
      attrs[:raw_retention_class] || attrs["raw_retention_class"] ||
        (policy && policy.raw_retention_class) || "hot"

    chunk_retention_class =
      attrs[:chunk_retention_class] || attrs["chunk_retention_class"] ||
        (policy && policy.chunk_retention_class) || "durable"

    raw_ttl_seconds =
      attrs[:raw_ttl_seconds] || attrs["raw_ttl_seconds"] || (policy && policy.raw_ttl_seconds)

    chunk_ttl_seconds =
      attrs[:chunk_ttl_seconds] || attrs["chunk_ttl_seconds"] ||
        (policy && policy.chunk_ttl_seconds)

    %{
      event_class: event_class,
      raw_retention_class: normalize_retention_class(raw_retention_class),
      chunk_retention_class: normalize_retention_class(chunk_retention_class),
      raw_expires_at: expiry_from_ttl(raw_ttl_seconds, now),
      chunk_expires_at: expiry_from_ttl(chunk_ttl_seconds, now)
    }
  end

  defp normalize_retention_class(value) when value in @retention_classes, do: value

  defp normalize_retention_class(value) when is_atom(value),
    do: normalize_retention_class(Atom.to_string(value))

  defp normalize_retention_class(_), do: "hot"

  defp expires_at_for(event_class, retention_class, base_time, type) do
    policy = get_retention_policy(event_class)
    retain_forever? = is_struct(policy, RetentionPolicy) and policy.retain_forever == true

    cond do
      retain_forever? or retention_class == "archive" ->
        nil

      type == :raw ->
        expiry_from_ttl(policy && policy.raw_ttl_seconds, base_time)

      true ->
        expiry_from_ttl(policy && policy.chunk_ttl_seconds, base_time)
    end
  end

  defp expiry_from_ttl(nil, _base_time), do: nil

  defp expiry_from_ttl(ttl_seconds, base_time) when is_integer(ttl_seconds) and ttl_seconds > 0,
    do: DateTime.add(base_time, ttl_seconds, :second)

  defp expiry_from_ttl(_ttl_seconds, _base_time), do: nil
end
