defmodule OpenAgentsRuntime.Runs.Executor do
  @moduledoc """
  Core run executor loop with lease enforcement and deterministic run transitions.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Cancel
  alias OpenAgentsRuntime.Runs.Frames
  alias OpenAgentsRuntime.Runs.Leases
  alias OpenAgentsRuntime.Runs.LoopDetection
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Runs.RunFrame
  alias OpenAgentsRuntime.Telemetry.Events
  alias OpenAgentsRuntime.Telemetry.Tracing

  @default_lease_ttl_seconds 30
  @terminal_statuses MapSet.new(["canceled", "succeeded", "failed"])

  @type run_once_opt ::
          {:lease_owner, String.t()}
          | {:lease_ttl_seconds, pos_integer()}
          | {:loop_detection, keyword() | map()}
          | {:now, DateTime.t()}

  @type run_result :: %{
          processed_frames: non_neg_integer(),
          status: String.t(),
          terminal_reason_class: String.t() | nil
        }

  @spec run_once(String.t(), [run_once_opt()]) :: {:ok, run_result()} | {:error, term()}
  def run_once(run_id, opts \\ []) when is_binary(run_id) do
    lease_owner = Keyword.get(opts, :lease_owner, default_lease_owner())
    lease_ttl_seconds = Keyword.get(opts, :lease_ttl_seconds, @default_lease_ttl_seconds)
    loop_detection_opts = normalize_loop_detection_opts(Keyword.get(opts, :loop_detection, []))
    now = Keyword.get(opts, :now, DateTime.utc_now())

    Tracing.with_phase_span(:infer, %{run_id: run_id, lease_owner: lease_owner}, fn ->
      started_at = System.monotonic_time()

      result =
        with %Run{} = run <- Repo.get(Run, run_id),
             {:ok, _lease} <-
               Leases.acquire(run_id, lease_owner,
                 now: now,
                 ttl_seconds: lease_ttl_seconds,
                 observed_progress_seq: run.latest_seq || 0
               ),
             {:ok, run} <- maybe_mark_started(run, lease_owner),
             {:ok, result} <-
               loop(run, lease_owner, lease_ttl_seconds, loop_detection_opts, 0) do
          {:ok, result}
        else
          nil -> {:error, :run_not_found}
          {:error, _reason} = error -> error
        end

      emit_run_once_telemetry(run_id, started_at, result)
      result
    end)
  end

  defp loop(%Run{} = run, lease_owner, lease_ttl_seconds, loop_detection_opts, processed_frames) do
    cond do
      terminal_status?(run.status) ->
        {:ok,
         %{
           processed_frames: processed_frames,
           status: run.status,
           terminal_reason_class: run.terminal_reason_class
         }}

      cancel_requested?(run) ->
        with {:ok, canceled_run} <- transition_to_canceled(run, lease_owner) do
          {:ok,
           %{
             processed_frames: processed_frames,
             status: canceled_run.status,
             terminal_reason_class: canceled_run.terminal_reason_class
           }}
        end

      true ->
        case Frames.next_pending_frame(run.run_id, run.last_processed_frame_id || 0) do
          nil ->
            {:ok,
             %{
               processed_frames: processed_frames,
               status: run.status,
               terminal_reason_class: run.terminal_reason_class
             }}

          %RunFrame{} = frame ->
            with {:ok, _lease} <-
                   Leases.renew(run.run_id, lease_owner, ttl_seconds: lease_ttl_seconds),
                 {:ok, next_run, terminal?} <-
                   process_frame(run, frame, lease_owner, loop_detection_opts) do
              if terminal? do
                {:ok,
                 %{
                   processed_frames: processed_frames + 1,
                   status: next_run.status,
                   terminal_reason_class: next_run.terminal_reason_class
                 }}
              else
                loop(
                  next_run,
                  lease_owner,
                  lease_ttl_seconds,
                  loop_detection_opts,
                  processed_frames + 1
                )
              end
            end
        end
    end
  end

  defp maybe_mark_started(%Run{status: "created"} = run, lease_owner) do
    with {:ok, event} <- RunEvents.append_event(run.run_id, "run.started", %{}),
         {:ok, run} <- update_run(run, %{status: "running"}),
         {:ok, _lease} <- Leases.mark_progress(run.run_id, lease_owner, event.seq) do
      emit_run_started_telemetry(run.run_id)
      {:ok, run}
    end
  end

  defp maybe_mark_started(%Run{} = run, _lease_owner), do: {:ok, run}

  defp process_frame(%Run{} = run, %RunFrame{} = frame, lease_owner, loop_detection_opts) do
    if frame_already_consumed?(run.run_id, frame.frame_id) do
      with {:ok, run} <- advance_cursor(run, frame.id) do
        emit_frame_telemetry(run.run_id, frame.frame_type, true)
        {:ok, run, terminal_status?(run.status)}
      end
    else
      with {:ok, loop_detection} <- LoopDetection.detect(run.run_id, frame, loop_detection_opts) do
        if loop_detection do
          handle_loop_detected(run, frame, lease_owner, loop_detection)
        else
          case classify_frame(frame) do
            {:continue, event_type, payload} ->
              with {:ok, event} <- RunEvents.append_event(run.run_id, event_type, payload),
                   {:ok, run} <- advance_cursor(run, frame.id),
                   {:ok, _lease} <- Leases.mark_progress(run.run_id, lease_owner, event.seq) do
                emit_frame_telemetry(run.run_id, frame.frame_type, false)
                {:ok, run, false}
              end

            {:terminal, status, reason_class, reason, events} ->
              with {:ok, last_seq} <- append_events(run.run_id, events),
                   {:ok, run} <-
                     persist_terminal(run, %{
                       status: status,
                       terminal_reason_class: reason_class,
                       terminal_reason: reason,
                       terminal_at: DateTime.utc_now(),
                       last_processed_frame_id: frame.id
                     }),
                   {:ok, _lease} <- Leases.mark_progress(run.run_id, lease_owner, last_seq) do
                emit_terminal_telemetry(run.run_id, status, reason_class)
                {:ok, run, true}
              end
          end
        end
      end
    end
  end

  defp handle_loop_detected(%Run{} = run, %RunFrame{} = frame, lease_owner, detection) do
    detector = detection.detector |> to_string()

    events = [
      {"run.loop_detected",
       %{
         "frame_id" => frame.frame_id,
         "frame_type" => frame.frame_type,
         "detector" => detector,
         "level" => to_string(detection.level),
         "count" => detection.count,
         "reason_code" => detection.reason_code,
         "loop_detected_reason" => detection.reason_code,
         "message" => detection.message
       }},
      {"run.finished",
       %{
         "status" => "failed",
         "reason_class" => "loop_detected",
         "reason" => detection.message,
         "reason_code" => detection.reason_code,
         "detector" => detector,
         "frame_id" => frame.frame_id,
         "loop_detected_reason" => detection.reason_code
       }}
    ]

    with {:ok, last_seq} <- append_events(run.run_id, events),
         {:ok, run} <-
           persist_terminal(run, %{
             status: "failed",
             terminal_reason_class: "loop_detected",
             terminal_reason: detection.message,
             terminal_at: DateTime.utc_now(),
             last_processed_frame_id: frame.id
           }),
         {:ok, _lease} <- Leases.mark_progress(run.run_id, lease_owner, last_seq) do
      emit_terminal_telemetry(run.run_id, "failed", "loop_detected")
      {:ok, run, true}
    end
  end

  defp classify_frame(%RunFrame{} = frame) do
    base_payload = %{
      "frame_id" => frame.frame_id,
      "frame_type" => frame.frame_type,
      "payload" => frame.payload
    }

    payload = stringify_map(frame.payload)

    case frame.frame_type do
      type when type in ["cancel", "cancel_requested"] ->
        reason = payload["reason"] || "cancel requested"

        events = [
          {"run.cancel_requested",
           Map.merge(base_payload, %{"reason_class" => "cancel_requested", "reason" => reason})},
          {"run.finished",
           %{
             "status" => "canceled",
             "reason_class" => "cancel_requested",
             "reason" => reason,
             "frame_id" => frame.frame_id
           }}
        ]

        {:terminal, "canceled", "cancel_requested", reason, events}

      type when type in ["finish", "complete", "run_complete"] ->
        reason = payload["reason"] || "completed"

        events = [
          {"run.finished",
           %{
             "status" => "succeeded",
             "reason_class" => "completed",
             "reason" => reason,
             "frame_id" => frame.frame_id
           }}
        ]

        {:terminal, "succeeded", "completed", reason, events}

      type when type in ["fail", "error"] ->
        reason_class = payload["reason_class"] || "execution_failed"
        reason = payload["reason"] || reason_class

        events = [
          {"run.finished",
           %{
             "status" => "failed",
             "reason_class" => reason_class,
             "reason" => reason,
             "frame_id" => frame.frame_id
           }}
        ]

        {:terminal, "failed", reason_class, reason, events}

      _ ->
        case payload["text"] || payload["delta"] do
          value when is_binary(value) and value != "" ->
            {:continue, "run.delta", %{"delta" => value, "frame_id" => frame.frame_id}}

          _ ->
            {:continue, "run.frame_consumed", base_payload}
        end
    end
  end

  defp append_events(run_id, events) do
    Enum.reduce_while(events, {:ok, 0}, fn {event_type, payload}, _acc ->
      case RunEvents.append_event(run_id, event_type, payload) do
        {:ok, event} -> {:cont, {:ok, event.seq}}
        {:error, reason} -> {:halt, {:error, reason}}
      end
    end)
  end

  defp cancel_requested?(%Run{} = run) do
    run.status == "canceling" or Cancel.cancel_requested?(run.run_id)
  end

  defp transition_to_canceled(%Run{} = run, lease_owner) do
    reason = latest_cancel_reason(run.run_id)

    with {:ok, last_seq} <- maybe_append_canceled_finish(run.run_id, reason),
         {:ok, run} <-
           persist_terminal(run, %{
             status: "canceled",
             terminal_reason_class: "cancel_requested",
             terminal_reason: reason,
             terminal_at: DateTime.utc_now()
           }),
         {:ok, _lease} <- Leases.mark_progress(run.run_id, lease_owner, last_seq) do
      :ok = OpenAgentsRuntime.Tools.ToolRunner.cancel_run(run.run_id)
      emit_terminal_telemetry(run.run_id, "canceled", "cancel_requested")
      {:ok, run}
    end
  end

  defp maybe_append_canceled_finish(run_id, reason) do
    if canceled_finish_exists?(run_id) do
      {:ok, RunEvents.latest_seq(run_id)}
    else
      with {:ok, event} <-
             RunEvents.append_event(run_id, "run.finished", %{
               "status" => "canceled",
               "reason_class" => "cancel_requested",
               "reason" => reason
             }) do
        {:ok, event.seq}
      end
    end
  end

  defp canceled_finish_exists?(run_id) do
    query =
      from(event in RunEvent,
        where:
          event.run_id == ^run_id and event.event_type == "run.finished" and
            fragment("?->>'status' = 'canceled'", event.payload),
        select: 1,
        limit: 1
      )

    Repo.exists?(query)
  end

  defp latest_cancel_reason(run_id) do
    query =
      from(event in RunEvent,
        where: event.run_id == ^run_id and event.event_type == "run.cancel_requested",
        order_by: [desc: event.seq],
        select: event.payload,
        limit: 1
      )

    case Repo.one(query) do
      %{"reason" => reason} when is_binary(reason) and reason != "" -> reason
      _ -> "cancel requested"
    end
  end

  defp frame_already_consumed?(run_id, frame_id) do
    query =
      from(event in RunEvent,
        where:
          event.run_id == ^run_id and fragment("?->>'frame_id' = ?", event.payload, ^frame_id),
        select: 1,
        limit: 1
      )

    Repo.exists?(query)
  end

  defp advance_cursor(%Run{} = run, frame_row_id)
       when is_integer(frame_row_id) and frame_row_id > 0 do
    if frame_row_id > (run.last_processed_frame_id || 0) do
      update_run(run, %{last_processed_frame_id: frame_row_id})
    else
      {:ok, run}
    end
  end

  defp persist_terminal(%Run{} = run, attrs) do
    update_run(run, attrs)
  end

  defp update_run(%Run{} = run, attrs) when is_map(attrs) do
    run
    |> Ecto.Changeset.change(attrs)
    |> Repo.update()
  end

  defp emit_frame_telemetry(run_id, frame_type, duplicate?) do
    Events.emit(
      [:openagents_runtime, :executor, :frame_processed],
      %{count: 1},
      %{run_id: run_id, frame_type: frame_type, duplicate: duplicate?}
    )
  end

  defp emit_terminal_telemetry(run_id, status, reason_class) do
    Events.emit(
      [:openagents_runtime, :executor, :terminal],
      %{count: 1},
      %{run_id: run_id, status: status, reason_class: reason_class || "none"}
    )
  end

  defp emit_run_started_telemetry(run_id) do
    Events.emit(
      [:openagents_runtime, :executor, :run_started],
      %{count: 1},
      %{run_id: run_id, status: "running"}
    )
  end

  defp emit_run_once_telemetry(run_id, started_at, result) do
    duration_ms = elapsed_ms(started_at)

    {status, reason_class, metadata} =
      case result do
        {:ok, run_result} ->
          status = run_result.status || "unknown"
          reason_class = run_result.terminal_reason_class || "none"
          {"ok", reason_class, %{status: status}}

        {:error, reason} ->
          {"error", normalize_reason_class(reason), %{error_detail: inspect(reason)}}
      end

    metadata =
      metadata
      |> Map.put(:run_id, run_id)
      |> Map.put(:result, status)
      |> Map.put(:reason_class, reason_class)

    Events.emit(
      [:openagents_runtime, :executor, :run_once],
      %{count: 1, duration_ms: duration_ms},
      metadata
    )
  end

  defp elapsed_ms(started_at) when is_integer(started_at) do
    (System.monotonic_time() - started_at)
    |> System.convert_time_unit(:native, :millisecond)
    |> max(0)
  end

  defp normalize_reason_class(:run_not_found), do: "run_not_found"
  defp normalize_reason_class(:lease_held), do: "lease_held"
  defp normalize_reason_class(:lease_progressed), do: "lease_progressed"
  defp normalize_reason_class(:not_owner), do: "not_owner"
  defp normalize_reason_class(reason) when is_atom(reason), do: Atom.to_string(reason)

  defp normalize_reason_class({:error, reason}), do: normalize_reason_class(reason)

  defp normalize_reason_class({:unsupported_strategy, _}), do: "unsupported_strategy"
  defp normalize_reason_class({:artifact_incompatible, _}), do: "artifact_incompatible"
  defp normalize_reason_class(_), do: "executor_error"

  defp stringify_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp normalize_loop_detection_opts(nil), do: []
  defp normalize_loop_detection_opts(opts) when is_list(opts), do: opts

  defp normalize_loop_detection_opts(opts) when is_map(opts) do
    opts
    |> Enum.map(fn
      {key, value} when is_binary(key) -> {to_existing_atom_safely(key), value}
      pair -> pair
    end)
    |> Enum.reject(fn {key, _value} -> is_nil(key) end)
  end

  defp normalize_loop_detection_opts(_opts), do: []

  defp to_existing_atom_safely(key) when is_binary(key) do
    try do
      String.to_existing_atom(key)
    rescue
      ArgumentError -> nil
    end
  end

  defp default_lease_owner do
    node_id = Node.self() |> to_string()
    "#{node_id}:#{inspect(self())}"
  end

  defp terminal_status?(status) when is_binary(status) do
    MapSet.member?(@terminal_statuses, status)
  end

  defp terminal_status?(_), do: false
end
