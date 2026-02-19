defmodule OpenAgentsRuntime.Runs.Janitor do
  @moduledoc """
  Reconciles stale run executors using durable lifecycle events and bounded retries.
  """

  use GenServer

  import Ecto.Query

  alias OpenAgentsRuntime.AgentProcess
  alias OpenAgentsRuntime.AgentSupervisor
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Runs.RunLease

  @default_scan_interval_ms 5_000
  @default_max_recovery_attempts 3
  @default_recovery_cooldown_ms 30_000
  @failure_reason_class "executor_recovery_exhausted"
  @terminal_statuses MapSet.new(["canceled", "succeeded", "failed"])

  @type run_once_opt ::
          {:now, DateTime.t()}
          | {:max_recovery_attempts, non_neg_integer()}
          | {:recovery_cooldown_ms, non_neg_integer()}

  @type reconcile_summary :: %{
          scanned: non_neg_integer(),
          resumed: non_neg_integer(),
          failed: non_neg_integer(),
          skipped: non_neg_integer()
        }

  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @spec run_once([run_once_opt()]) :: reconcile_summary()
  def run_once(opts \\ []) do
    now = Keyword.get(opts, :now, DateTime.utc_now())

    do_reconcile(now,
      max_recovery_attempts: Keyword.get(opts, :max_recovery_attempts, max_recovery_attempts()),
      recovery_cooldown_ms: Keyword.get(opts, :recovery_cooldown_ms, recovery_cooldown_ms())
    )
  end

  @impl true
  def init(_opts) do
    state = %{scan_interval_ms: scan_interval_ms()}
    Process.send_after(self(), :reconcile, state.scan_interval_ms)
    {:ok, state}
  end

  @impl true
  def handle_info(:reconcile, state) do
    _summary = run_once()
    Process.send_after(self(), :reconcile, state.scan_interval_ms)
    {:noreply, state}
  end

  defp do_reconcile(now, opts) do
    max_attempts = Keyword.fetch!(opts, :max_recovery_attempts)
    cooldown_ms = Keyword.fetch!(opts, :recovery_cooldown_ms)

    stale_runs = stale_runs(now)

    Enum.reduce(stale_runs, %{scanned: 0, resumed: 0, failed: 0, skipped: 0}, fn {run, lease},
                                                                                 acc ->
      acc = %{acc | scanned: acc.scanned + 1}

      case reconcile_stale_run(run, lease, now, max_attempts, cooldown_ms) do
        :resumed -> %{acc | resumed: acc.resumed + 1}
        :failed -> %{acc | failed: acc.failed + 1}
        :skipped -> %{acc | skipped: acc.skipped + 1}
      end
    end)
  end

  defp stale_runs(now) do
    query =
      from(run in Run,
        join: lease in RunLease,
        on: lease.run_id == run.run_id,
        where:
          lease.lease_expires_at < ^now and run.status not in ^MapSet.to_list(@terminal_statuses),
        select: {run, lease}
      )

    Repo.all(query)
  end

  defp reconcile_stale_run(run, lease, now, max_attempts, cooldown_ms) do
    if should_attempt_recovery?(run, now, cooldown_ms) do
      attempt_count = (run.recovery_attempt_count || 0) + 1

      with {:ok, _event} <- append_executor_lost_event(run, lease, now),
           {:ok, run} <-
             update_run(run, %{recovery_attempt_count: attempt_count, last_recovery_at: now}) do
        if attempt_count > max_attempts do
          mark_failed_recovery(run, now)
          :failed
        else
          resume_run(run)
          :resumed
        end
      else
        _ -> :skipped
      end
    else
      :skipped
    end
  end

  defp append_executor_lost_event(run, lease, now) do
    payload = %{
      "lease_owner" => lease.lease_owner,
      "lease_expires_at" => DateTime.to_iso8601(lease.lease_expires_at),
      "detected_at" => DateTime.to_iso8601(now),
      "last_progress_seq" => lease.last_progress_seq
    }

    RunEvents.append_event(run.run_id, "run.executor_lost", payload)
  end

  defp mark_failed_recovery(run, now) do
    reason = "janitor recovery attempts exceeded"

    _ = maybe_append_failed_finish(run.run_id, reason)

    _ =
      update_run(run, %{
        status: "failed",
        terminal_reason_class: @failure_reason_class,
        terminal_reason: reason,
        terminal_at: now,
        last_recovery_at: now
      })

    :telemetry.execute(
      [:openagents_runtime, :janitor, :failed],
      %{count: 1},
      %{run_id: run.run_id, reason_class: @failure_reason_class}
    )
  end

  defp maybe_append_failed_finish(run_id, reason) do
    if failed_finish_exists?(run_id) do
      :ok
    else
      case RunEvents.append_event(run_id, "run.finished", %{
             "status" => "failed",
             "reason_class" => @failure_reason_class,
             "reason" => reason
           }) do
        {:ok, _event} -> :ok
        _ -> :error
      end
    end
  end

  defp failed_finish_exists?(run_id) do
    query =
      from(event in RunEvent,
        where:
          event.run_id == ^run_id and event.event_type == "run.finished" and
            fragment("?->>'reason_class' = ?", event.payload, ^@failure_reason_class),
        select: 1,
        limit: 1
      )

    Repo.exists?(query)
  end

  defp resume_run(run) do
    with {:ok, _pid} <- AgentSupervisor.ensure_agent(run.run_id),
         :ok <- AgentProcess.resume(run.run_id) do
      :telemetry.execute(
        [:openagents_runtime, :janitor, :resumed],
        %{count: 1},
        %{run_id: run.run_id}
      )

      :ok
    else
      _ -> :error
    end
  end

  defp should_attempt_recovery?(run, now, cooldown_ms) do
    case run.last_recovery_at do
      nil ->
        true

      %DateTime{} = last_recovery_at ->
        elapsed_ms = DateTime.diff(now, last_recovery_at, :millisecond)
        elapsed_ms >= cooldown_ms
    end
  end

  defp update_run(run, attrs) do
    run
    |> Ecto.Changeset.change(attrs)
    |> Repo.update()
  end

  defp scan_interval_ms do
    Application.get_env(:openagents_runtime, :janitor_scan_interval_ms, @default_scan_interval_ms)
  end

  defp max_recovery_attempts do
    Application.get_env(
      :openagents_runtime,
      :janitor_max_recovery_attempts,
      @default_max_recovery_attempts
    )
  end

  defp recovery_cooldown_ms do
    Application.get_env(
      :openagents_runtime,
      :janitor_recovery_cooldown_ms,
      @default_recovery_cooldown_ms
    )
  end
end
