defmodule OpenAgentsRuntime.AgentProcess do
  @moduledoc """
  Per-run process that coordinates executor runs under supervision.

  The GenServer only schedules work and tracks lifecycle state; heavy execution is
  offloaded to a supervised task.
  """

  use GenServer

  alias OpenAgentsRuntime.AgentRegistry
  alias OpenAgentsRuntime.Runs.Executor

  @idle_shutdown_ms 30_000

  @type state :: %{
          run_id: String.t(),
          last_frame_id: String.t() | nil,
          frame_count: non_neg_integer(),
          pending_execution: boolean(),
          executing_ref: reference() | nil,
          executing_pid: pid() | nil,
          last_result: term() | nil,
          last_error: term() | nil,
          idle_timer_ref: reference() | nil
        }

  @spec child_spec(String.t()) :: Supervisor.child_spec()
  def child_spec(run_id) when is_binary(run_id) do
    %{
      id: {__MODULE__, run_id},
      start: {__MODULE__, :start_link, [run_id]},
      restart: :transient,
      shutdown: 5_000,
      type: :worker
    }
  end

  @spec start_link(String.t()) :: GenServer.on_start()
  def start_link(run_id) when is_binary(run_id) do
    GenServer.start_link(__MODULE__, run_id, name: AgentRegistry.via(run_id))
  end

  @spec ingest_frame(String.t(), String.t()) :: :ok | {:error, :agent_not_running}
  def ingest_frame(run_id, frame_id) when is_binary(run_id) and is_binary(frame_id) do
    case AgentRegistry.whereis(run_id) do
      nil ->
        {:error, :agent_not_running}

      pid when is_pid(pid) ->
        GenServer.cast(pid, {:ingest_frame, frame_id})
        :ok
    end
  end

  @spec cancel(String.t()) :: :ok | {:error, :agent_not_running}
  def cancel(run_id) when is_binary(run_id) do
    case AgentRegistry.whereis(run_id) do
      nil ->
        {:error, :agent_not_running}

      pid when is_pid(pid) ->
        GenServer.cast(pid, :cancel)
        :ok
    end
  end

  @spec resume(String.t()) :: :ok | {:error, :agent_not_running}
  def resume(run_id) when is_binary(run_id) do
    case AgentRegistry.whereis(run_id) do
      nil ->
        {:error, :agent_not_running}

      pid when is_pid(pid) ->
        GenServer.cast(pid, :resume)
        :ok
    end
  end

  @spec snapshot(String.t()) :: {:ok, state()} | {:error, :agent_not_running}
  def snapshot(run_id) when is_binary(run_id) do
    case AgentRegistry.whereis(run_id) do
      nil -> {:error, :agent_not_running}
      pid when is_pid(pid) -> {:ok, GenServer.call(pid, :snapshot)}
    end
  end

  @spec crash(String.t()) :: :ok | {:error, :agent_not_running}
  def crash(run_id) when is_binary(run_id) do
    case AgentRegistry.whereis(run_id) do
      nil ->
        {:error, :agent_not_running}

      pid when is_pid(pid) ->
        GenServer.call(pid, :crash)
    end
  end

  @impl true
  def init(run_id) do
    state = %{
      run_id: run_id,
      last_frame_id: nil,
      frame_count: 0,
      pending_execution: false,
      executing_ref: nil,
      executing_pid: nil,
      last_result: nil,
      last_error: nil,
      idle_timer_ref: nil
    }

    {:ok, schedule_idle_shutdown(state)}
  end

  @impl true
  def handle_cast({:ingest_frame, frame_id}, state) do
    state =
      state
      |> cancel_idle_shutdown()
      |> Map.put(:last_frame_id, frame_id)
      |> Map.update!(:frame_count, &(&1 + 1))
      |> Map.put(:pending_execution, true)

    state = maybe_schedule_execute(state)
    emit_process_stats(state.run_id, :ingest_frame)

    {:noreply, state}
  end

  def handle_cast(:cancel, state) do
    :ok = OpenAgentsRuntime.Tools.ToolRunner.cancel_run(state.run_id)

    state =
      state
      |> cancel_idle_shutdown()
      |> Map.put(:pending_execution, true)
      |> maybe_schedule_execute()

    emit_process_stats(state.run_id, :cancel)
    {:noreply, state}
  end

  def handle_cast(:resume, state) do
    state =
      state
      |> cancel_idle_shutdown()
      |> Map.put(:pending_execution, true)
      |> maybe_schedule_execute()

    emit_process_stats(state.run_id, :resume)
    {:noreply, state}
  end

  @impl true
  def handle_call(:snapshot, _from, state) do
    emit_process_stats(state.run_id, :snapshot)
    {:reply, state, state}
  end

  def handle_call(:crash, _from, state) do
    raise "agent process crash requested for #{state.run_id}"
  end

  @impl true
  def handle_info(:execute, state) do
    state =
      case {state.pending_execution, state.executing_ref} do
        {true, nil} ->
          state = Map.put(state, :pending_execution, false)
          start_executor_task(state)

        _ ->
          state
      end

    emit_process_stats(state.run_id, :execute)
    {:noreply, state}
  end

  def handle_info({ref, result}, %{executing_ref: ref} = state) do
    Process.demonitor(ref, [:flush])

    state =
      state
      |> Map.put(:executing_ref, nil)
      |> Map.put(:executing_pid, nil)
      |> Map.put(:last_result, result)
      |> Map.put(:last_error, execution_error(result))
      |> maybe_schedule_execute()
      |> maybe_schedule_idle_shutdown()

    emit_process_stats(state.run_id, :executor_result)
    {:noreply, state}
  end

  def handle_info({:DOWN, ref, :process, _pid, reason}, %{executing_ref: ref} = state) do
    state =
      state
      |> Map.put(:executing_ref, nil)
      |> Map.put(:executing_pid, nil)
      |> Map.put(:last_error, down_reason(reason))
      |> maybe_schedule_execute()
      |> maybe_schedule_idle_shutdown()

    emit_process_stats(state.run_id, :executor_down)
    {:noreply, state}
  end

  def handle_info(:idle_shutdown, state) do
    emit_process_stats(state.run_id, :idle_shutdown)

    if state.pending_execution or state.executing_ref do
      {:noreply, maybe_schedule_idle_shutdown(state)}
    else
      {:stop, :normal, state}
    end
  end

  def handle_info(_message, state) do
    {:noreply, state}
  end

  defp start_executor_task(state) do
    run_id = state.run_id
    trace_context = OpenAgentsRuntime.Telemetry.Tracing.current()

    task =
      Task.Supervisor.async_nolink(OpenAgentsRuntime.Tools.TaskSupervisor, fn ->
        :ok = OpenAgentsRuntime.Telemetry.Tracing.put_current(trace_context)
        Executor.run_once(run_id, lease_owner: lease_owner(run_id))
      end)

    state
    |> Map.put(:executing_ref, task.ref)
    |> Map.put(:executing_pid, task.pid)
  end

  defp maybe_schedule_execute(%{pending_execution: true, executing_ref: nil} = state) do
    Process.send(self(), :execute, [])
    state
  end

  defp maybe_schedule_execute(state), do: state

  defp schedule_idle_shutdown(state) do
    timer = Process.send_after(self(), :idle_shutdown, idle_shutdown_ms())
    Map.put(state, :idle_timer_ref, timer)
  end

  defp maybe_schedule_idle_shutdown(state) do
    if state.pending_execution or state.executing_ref do
      state
    else
      state
      |> cancel_idle_shutdown()
      |> schedule_idle_shutdown()
    end
  end

  defp cancel_idle_shutdown(%{idle_timer_ref: nil} = state), do: state

  defp cancel_idle_shutdown(%{idle_timer_ref: timer} = state) do
    _ = Process.cancel_timer(timer)
    Map.put(state, :idle_timer_ref, nil)
  end

  defp idle_shutdown_ms do
    Application.get_env(:openagents_runtime, :agent_process_idle_shutdown_ms, @idle_shutdown_ms)
  end

  defp lease_owner(run_id) do
    node_id = Node.self() |> to_string()
    "#{node_id}:agent:#{run_id}:#{inspect(self())}"
  end

  defp execution_error({:ok, _}), do: nil
  defp execution_error({:error, reason}), do: reason
  defp execution_error(other), do: other

  defp down_reason(:normal), do: nil
  defp down_reason(:shutdown), do: nil
  defp down_reason({:shutdown, _}), do: nil
  defp down_reason(reason), do: reason

  defp emit_process_stats(run_id, event) do
    process_info =
      self()
      |> Process.info([:message_queue_len, :reductions])
      |> Enum.into(%{})

    :telemetry.execute(
      [:openagents_runtime, :agent_process, :stats],
      %{
        message_queue_len: process_info[:message_queue_len] || 0,
        reductions: process_info[:reductions] || 0
      },
      %{run_id: run_id, event: event}
    )
  end
end
