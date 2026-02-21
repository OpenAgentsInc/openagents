defmodule OpenAgentsRuntime.Memory.Rollup do
  @moduledoc """
  L2/L3 rollup and bounded expansion APIs for timeline memory chunks.
  """

  alias OpenAgentsRuntime.Memory.MemoryChunk
  alias OpenAgentsRuntime.Memory.TimelineStore

  @default_l2_chunk_count 6
  @default_l3_chunk_count 24
  @default_expand_max_depth 2
  @default_expand_max_items 50
  @allowed_modes ~w(interactive delegated_budget system)

  @type rollup_opt ::
          {:chunk_count, pos_integer()}
          | {:trigger, :scheduled | :pressure | :manual}
          | {:event_class, String.t()}
          | {:artifact_uri, String.t() | nil}

  @type expand_opt ::
          {:max_depth, non_neg_integer()}
          | {:max_items, pos_integer()}
          | {:authorization_mode, String.t()}

  @spec rollup_l2(String.t(), [rollup_opt()]) :: {:ok, map()} | {:error, term()}
  def rollup_l2(run_id, opts \\ []) when is_binary(run_id) do
    rollup(run_id, 1, 2, Keyword.get(opts, :chunk_count, @default_l2_chunk_count), opts)
  end

  @spec rollup_l3(String.t(), [rollup_opt()]) :: {:ok, map()} | {:error, term()}
  def rollup_l3(run_id, opts \\ []) when is_binary(run_id) do
    rollup(run_id, 2, 3, Keyword.get(opts, :chunk_count, @default_l3_chunk_count), opts)
  end

  @spec expand_chunk(String.t(), String.t(), [expand_opt()]) :: {:ok, map()} | {:error, term()}
  def expand_chunk(run_id, chunk_id, opts \\ []) when is_binary(run_id) and is_binary(chunk_id) do
    authorization_mode = Keyword.get(opts, :authorization_mode, "interactive")

    if authorization_mode in @allowed_modes do
      max_depth = Keyword.get(opts, :max_depth, @default_expand_max_depth)
      max_items = Keyword.get(opts, :max_items, @default_expand_max_items)

      case TimelineStore.get_chunk(run_id, chunk_id) do
        nil ->
          {:error, :chunk_not_found}

        chunk ->
          {:ok, expand_chunk_tree(run_id, chunk, max_depth, max_items)}
      end
    else
      {:error, :expansion_not_allowed}
    end
  end

  defp rollup(run_id, source_level, target_level, chunk_count, opts) do
    event_class = Keyword.get(opts, :event_class, "default")
    trigger = normalize_trigger(Keyword.get(opts, :trigger, :scheduled))
    artifact_uri = Keyword.get(opts, :artifact_uri)
    started_at = DateTime.utc_now()

    source_chunks =
      run_id
      |> TimelineStore.list_chunks(level: source_level, limit: chunk_count)
      |> Enum.reverse()

    if length(source_chunks) < 2 do
      rollup_id = "noop_#{target_level}_#{run_id}_#{System.unique_integer([:positive])}"

      _ =
        TimelineStore.insert_rollup(%{
          run_id: run_id,
          source_level: source_level,
          target_level: target_level,
          source_chunk_ids: Enum.map(source_chunks, & &1.chunk_id),
          output_chunk_id: rollup_id,
          status: "noop",
          metadata: %{"reason" => "insufficient_source_chunks", "trigger" => trigger},
          started_at: started_at,
          completed_at: DateTime.utc_now()
        })

      {:ok, %{status: :noop, source_count: length(source_chunks), output_chunk_id: nil}}
    else
      persist_rollup(
        run_id,
        source_chunks,
        source_level,
        target_level,
        trigger,
        event_class,
        artifact_uri,
        started_at
      )
    end
  end

  defp persist_rollup(
         run_id,
         source_chunks,
         source_level,
         target_level,
         trigger,
         event_class,
         artifact_uri,
         started_at
       ) do
    first_chunk = List.first(source_chunks)
    last_chunk = List.last(source_chunks)
    source_chunk_ids = Enum.map(source_chunks, & &1.chunk_id)
    summary_text = Enum.map_join(source_chunks, "\n", &chunk_summary_line/1)
    summary_hash = sha256(summary_text)

    output_chunk_id =
      "l#{target_level}_#{run_id}_#{first_chunk.chunk_id}_#{last_chunk.chunk_id}_#{String.slice(summary_hash, 0, 10)}"

    case TimelineStore.get_chunk(run_id, output_chunk_id) do
      %MemoryChunk{} = existing ->
        {:ok,
         %{
           status: :succeeded,
           source_count: length(source_chunks),
           output_chunk_id: existing.chunk_id,
           idempotent_replay: true
         }}

      nil ->
        rollup_summary = %{
          "kind" => "l#{target_level}_rollup",
          "source_level" => source_level,
          "target_level" => target_level,
          "source_chunk_ids" => source_chunk_ids,
          "summary_hash" => summary_hash,
          "summary_text" => truncate(summary_text, 3_000),
          "trigger" => trigger
        }

        token_count =
          source_chunks
          |> Enum.map(&(&1.token_count || 0))
          |> Enum.sum()
          |> max(1)

        with {:ok, _chunk} <-
               TimelineStore.insert_chunk(run_id, %{
                 chunk_id: output_chunk_id,
                 level: target_level,
                 event_class: event_class,
                 retention_class: "archive",
                 window_started_at: first_chunk.window_started_at,
                 window_ended_at: last_chunk.window_ended_at,
                 source_chunk_ids: source_chunk_ids,
                 summary: rollup_summary,
                 token_count: token_count,
                 storage_uri: artifact_uri
               }),
             {:ok, _rollup} <-
               TimelineStore.insert_rollup(%{
                 run_id: run_id,
                 source_level: source_level,
                 target_level: target_level,
                 source_chunk_ids: source_chunk_ids,
                 output_chunk_id: output_chunk_id,
                 summary_hash: summary_hash,
                 status: "succeeded",
                 metadata: %{"trigger" => trigger, "source_count" => length(source_chunks)},
                 started_at: started_at,
                 completed_at: DateTime.utc_now()
               }) do
          {:ok,
           %{
             status: :succeeded,
             source_count: length(source_chunks),
             output_chunk_id: output_chunk_id,
             idempotent_replay: false
           }}
        else
          {:error, reason} ->
            _ =
              TimelineStore.insert_rollup(%{
                run_id: run_id,
                source_level: source_level,
                target_level: target_level,
                source_chunk_ids: source_chunk_ids,
                output_chunk_id: output_chunk_id,
                summary_hash: summary_hash,
                status: "failed",
                metadata: %{"trigger" => trigger},
                error_message: inspect(reason),
                started_at: started_at,
                completed_at: DateTime.utc_now()
              })

            {:error, reason}
        end
    end
  end

  defp expand_chunk_tree(run_id, root_chunk, max_depth, max_items) do
    {tree, _remaining_items} = do_expand(run_id, root_chunk, max_depth, max_items)

    %{
      "chunk_id" => root_chunk.chunk_id,
      "level" => root_chunk.level,
      "window" => %{
        "started_at" => to_iso8601(root_chunk.window_started_at),
        "ended_at" => to_iso8601(root_chunk.window_ended_at)
      },
      "max_depth" => max_depth,
      "max_items" => max_items,
      "tree" => tree
    }
  end

  defp do_expand(_run_id, _chunk, _depth, remaining_items) when remaining_items <= 0 do
    {%{"truncated" => true}, 0}
  end

  defp do_expand(_run_id, chunk, 0, remaining_items) do
    {
      %{
        "chunk_id" => chunk.chunk_id,
        "level" => chunk.level,
        "summary" => chunk.summary,
        "source_chunk_ids" => chunk.source_chunk_ids || [],
        "truncated" => true
      },
      remaining_items - 1
    }
  end

  defp do_expand(run_id, chunk, depth, remaining_items) do
    remaining_items = remaining_items - 1
    source_ids = chunk.source_chunk_ids || []

    {children, remaining_items} =
      Enum.reduce_while(source_ids, {[], remaining_items}, fn source_chunk_id,
                                                              {acc, items_left} ->
        if items_left <= 0 do
          {:halt, {Enum.reverse(acc), 0}}
        else
          case TimelineStore.get_chunk(run_id, source_chunk_id) do
            nil ->
              child = %{"chunk_id" => source_chunk_id, "missing" => true}
              {:cont, {[child | acc], items_left - 1}}

            source_chunk ->
              {child_tree, items_left} = do_expand(run_id, source_chunk, depth - 1, items_left)
              {:cont, {[child_tree | acc], items_left}}
          end
        end
      end)

    tree = %{
      "chunk_id" => chunk.chunk_id,
      "level" => chunk.level,
      "summary" => chunk.summary,
      "children" => children
    }

    {tree, remaining_items}
  end

  defp chunk_summary_line(chunk) do
    hash = chunk.summary |> :erlang.term_to_binary() |> sha256() |> String.slice(0, 10)
    "#{chunk.chunk_id}|#{hash}|#{chunk.token_count || 0}"
  end

  defp normalize_trigger(:scheduled), do: "scheduled"
  defp normalize_trigger("scheduled"), do: "scheduled"
  defp normalize_trigger(:pressure), do: "pressure"
  defp normalize_trigger("pressure"), do: "pressure"
  defp normalize_trigger(:manual), do: "manual"
  defp normalize_trigger("manual"), do: "manual"
  defp normalize_trigger(_), do: "scheduled"

  defp sha256(value) when is_binary(value) do
    value
    |> then(&:crypto.hash(:sha256, &1))
    |> Base.encode16(case: :lower)
  end

  defp sha256(value), do: value |> :erlang.term_to_binary() |> sha256()

  defp to_iso8601(%DateTime{} = datetime), do: DateTime.to_iso8601(datetime)
  defp to_iso8601(nil), do: nil

  defp truncate(value, max_chars) when is_binary(value) do
    if String.length(value) <= max_chars do
      value
    else
      String.slice(value, 0, max_chars)
    end
  end
end
