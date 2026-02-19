defmodule OpenAgentsRuntime.Memory.L1Compactor do
  @moduledoc """
  L1 compaction flow for raw timeline events with durable audit artifacts.
  """

  alias OpenAgentsRuntime.Memory.TimelineStore
  alias OpenAgentsRuntime.Repo

  @max_summary_chars 3_000
  @default_max_events 60

  @type compact_opt ::
          {:trigger, :scheduled | :pressure}
          | {:max_events, pos_integer()}
          | {:event_class, String.t()}
          | {:model_name, String.t()}
          | {:model_version, String.t()}
          | {:artifact_uri, String.t() | nil}
          | {:upto_seq, non_neg_integer()}

  @type compact_result ::
          %{
            status: :noop | :succeeded | :failed,
            run_id: String.t(),
            input_event_count: non_neg_integer(),
            output_chunk_id: String.t() | nil,
            dropped_event_count: non_neg_integer()
          }

  @spec compact_l1(String.t(), [compact_opt()]) :: {:ok, compact_result()} | {:error, term()}
  def compact_l1(run_id, opts \\ []) when is_binary(run_id) do
    trigger = normalize_trigger(Keyword.get(opts, :trigger, :scheduled))
    max_events = Keyword.get(opts, :max_events, @default_max_events)
    event_class = Keyword.get(opts, :event_class, "default")
    model_name = Keyword.get(opts, :model_name, "runtime.l1_compactor")
    model_version = Keyword.get(opts, :model_version, "v1")
    artifact_uri = Keyword.get(opts, :artifact_uri)
    upto_seq = Keyword.get(opts, :upto_seq)
    started_at = DateTime.utc_now() |> DateTime.truncate(:microsecond)

    events =
      TimelineStore.list_raw_events(run_id,
        since_seq: 0,
        upto_seq: upto_seq,
        event_class: event_class,
        limit: max_events
      )

    case events do
      [] ->
        attrs = %{
          run_id: run_id,
          level: 1,
          trigger_type: trigger,
          status: "noop",
          input_event_count: 0,
          metadata: %{"reason" => "no_events"},
          started_at: started_at,
          completed_at: DateTime.utc_now()
        }

        _ = TimelineStore.insert_compaction(attrs)

        {:ok,
         %{
           status: :noop,
           run_id: run_id,
           input_event_count: 0,
           output_chunk_id: nil,
           dropped_event_count: 0
         }}

      _events ->
        Repo.transaction(fn ->
          build_and_persist_compaction(
            run_id,
            events,
            trigger,
            event_class,
            model_name,
            model_version,
            artifact_uri,
            started_at
          )
        end)
        |> case do
          {:ok, {:ok, result}} ->
            {:ok, result}

          {:ok, {:error, reason}} ->
            {:error, reason}

          {:error, reason} ->
            {:error, reason}
        end
    end
  end

  defp build_and_persist_compaction(
         run_id,
         events,
         trigger,
         event_class,
         model_name,
         model_version,
         artifact_uri,
         started_at
       ) do
    first_event = List.first(events)
    last_event = List.last(events)
    summary_text = summarize_events(events)
    summary_hash = sha256(summary_text)
    token_count_input = token_estimate(events)
    token_count_output = token_estimate(summary_text)

    chunk_id =
      "l1_#{run_id}_#{first_event.seq}_#{last_event.seq}_#{String.slice(summary_hash, 0, 12)}"

    chunk_summary = %{
      "kind" => "l1_compaction",
      "summary_text" => summary_text,
      "summary_hash" => summary_hash,
      "input_event_ids" => Enum.map(events, & &1.id),
      "input_event_seqs" => [first_event.seq, last_event.seq],
      "trigger" => trigger,
      "model" => %{"name" => model_name, "version" => model_version}
    }

    with {:ok, _chunk} <-
           TimelineStore.insert_chunk(run_id, %{
             chunk_id: chunk_id,
             level: 1,
             event_class: event_class,
             retention_class: "durable",
             window_started_at: first_event.occurred_at || first_event.inserted_at,
             window_ended_at: last_event.occurred_at || last_event.inserted_at,
             source_event_start_seq: first_event.seq,
             source_event_end_seq: last_event.seq,
             summary: chunk_summary,
             token_count: token_count_output,
             storage_uri: artifact_uri
           }),
         dropped_event_count <- TimelineStore.drop_raw_events_up_to(run_id, last_event.seq),
         {:ok, _compaction} <-
           TimelineStore.insert_compaction(%{
             run_id: run_id,
             level: 1,
             trigger_type: trigger,
             status: "succeeded",
             input_event_start_seq: first_event.seq,
             input_event_end_seq: last_event.seq,
             input_event_count: length(events),
             output_chunk_id: chunk_id,
             summary_hash: summary_hash,
             model_name: model_name,
             model_version: model_version,
             token_count_input: token_count_input,
             token_count_output: token_count_output,
             artifact_uri: artifact_uri,
             metadata: %{
               "input_event_ids" => Enum.map(events, & &1.id),
               "input_event_count" => length(events),
               "trigger" => trigger
             },
             started_at: started_at,
             completed_at: DateTime.utc_now()
           }) do
      {:ok,
       %{
         status: :succeeded,
         run_id: run_id,
         input_event_count: length(events),
         output_chunk_id: chunk_id,
         dropped_event_count: dropped_event_count
       }}
    else
      {:error, reason} ->
        _ =
          TimelineStore.insert_compaction(%{
            run_id: run_id,
            level: 1,
            trigger_type: trigger,
            status: "failed",
            input_event_start_seq: first_event.seq,
            input_event_end_seq: last_event.seq,
            input_event_count: length(events),
            summary_hash: summary_hash,
            model_name: model_name,
            model_version: model_version,
            token_count_input: token_count_input,
            token_count_output: token_count_output,
            artifact_uri: artifact_uri,
            metadata: %{"trigger" => trigger},
            error_message: inspect(reason),
            started_at: started_at,
            completed_at: DateTime.utc_now()
          })

        {:error, reason}
    end
  end

  defp summarize_events(events) do
    events
    |> Enum.map(fn event ->
      payload_hash = event.payload |> :erlang.term_to_binary() |> sha256() |> String.slice(0, 12)
      "#{event.seq}|#{event.event_type}|#{payload_hash}"
    end)
    |> Enum.join("\n")
    |> truncate(@max_summary_chars)
  end

  defp normalize_trigger(:scheduled), do: "scheduled"
  defp normalize_trigger("scheduled"), do: "scheduled"
  defp normalize_trigger(:pressure), do: "pressure"
  defp normalize_trigger("pressure"), do: "pressure"
  defp normalize_trigger(_), do: "scheduled"

  defp token_estimate(value) when is_binary(value) do
    value |> String.length() |> div(4) |> max(1)
  end

  defp token_estimate(value) do
    value
    |> :erlang.term_to_binary()
    |> byte_size()
    |> div(4)
    |> max(1)
  end

  defp sha256(value) when is_binary(value) do
    value
    |> then(&:crypto.hash(:sha256, &1))
    |> Base.encode16(case: :lower)
  end

  defp sha256(value), do: value |> :erlang.term_to_binary() |> sha256()

  defp truncate(value, max_chars) when is_binary(value) do
    if String.length(value) <= max_chars do
      value
    else
      String.slice(value, 0, max_chars)
    end
  end
end
