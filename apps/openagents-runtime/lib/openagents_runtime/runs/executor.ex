defmodule OpenAgentsRuntime.Runs.Executor do
  @moduledoc """
  Core run executor loop with lease enforcement and deterministic run transitions.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Cancel
  alias OpenAgentsRuntime.Runs.Frames
  alias OpenAgentsRuntime.Runs.Leases
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Runs.RunFrame
  alias OpenAgentsRuntime.Telemetry.Tracing

  @default_lease_ttl_seconds 30
  @terminal_statuses MapSet.new(["canceled", "succeeded", "failed"])

  @type run_once_opt ::
          {:lease_owner, String.t()}
          | {:lease_ttl_seconds, pos_integer()}
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
    now = Keyword.get(opts, :now, DateTime.utc_now())

    Tracing.with_phase_span(:infer, %{run_id: run_id, lease_owner: lease_owner}, fn ->
      with %Run{} = run <- Repo.get(Run, run_id),
           {:ok, _lease} <-
             Leases.acquire(run_id, lease_owner,
               now: now,
               ttl_seconds: lease_ttl_seconds,
               observed_progress_seq: run.latest_seq || 0
             ),
           {:ok, run} <- maybe_mark_started(run, lease_owner),
           {:ok, result} <- loop(run, lease_owner, lease_ttl_seconds, 0) do
        {:ok, result}
      else
        nil -> {:error, :run_not_found}
        {:error, _reason} = error -> error
      end
    end)
  end

  defp loop(%Run{} = run, lease_owner, lease_ttl_seconds, processed_frames) do
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
                 {:ok, next_run, terminal?} <- process_frame(run, frame, lease_owner) do
              if terminal? do
                {:ok,
                 %{
                   processed_frames: processed_frames + 1,
                   status: next_run.status,
                   terminal_reason_class: next_run.terminal_reason_class
                 }}
              else
                loop(next_run, lease_owner, lease_ttl_seconds, processed_frames + 1)
              end
            end
        end
    end
  end

  defp maybe_mark_started(%Run{status: "created"} = run, lease_owner) do
    with {:ok, event} <- RunEvents.append_event(run.run_id, "run.started", %{}),
         {:ok, run} <- update_run(run, %{status: "running"}),
         {:ok, _lease} <- Leases.mark_progress(run.run_id, lease_owner, event.seq) do
      {:ok, run}
    end
  end

  defp maybe_mark_started(%Run{} = run, _lease_owner), do: {:ok, run}

  defp process_frame(%Run{} = run, %RunFrame{} = frame, lease_owner) do
    if frame_already_consumed?(run.run_id, frame.frame_id) do
      with {:ok, run} <- advance_cursor(run, frame.id) do
        emit_frame_telemetry(run.run_id, frame.frame_type, true)
        {:ok, run, terminal_status?(run.status)}
      end
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
    :telemetry.execute(
      [:openagents_runtime, :executor, :frame_processed],
      %{count: 1},
      %{run_id: run_id, frame_type: frame_type, duplicate: duplicate?}
    )
  end

  defp emit_terminal_telemetry(run_id, status, reason_class) do
    :telemetry.execute(
      [:openagents_runtime, :executor, :terminal],
      %{count: 1},
      %{run_id: run_id, status: status, reason_class: reason_class}
    )
  end

  defp stringify_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
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
