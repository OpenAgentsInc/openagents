defmodule OpenAgentsRuntime.Tools.ToolRunner do
  @moduledoc """
  Executes tool work inside a supervised task boundary and supports
  best-effort run-scoped cancellation.
  """

  alias OpenAgentsRuntime.Telemetry.Tracing

  @default_timeout_ms 5_000

  @spec run((-> term()), timeout() | keyword()) :: {:ok, term()} | {:error, :timeout | :canceled}
  def run(fun, timeout_ms \\ @default_timeout_ms)

  def run(fun, timeout_ms) when is_function(fun, 0) and is_integer(timeout_ms) do
    run(fun, timeout_ms: timeout_ms)
  end

  def run(fun, opts) when is_function(fun, 0) and is_list(opts) do
    timeout_ms = Keyword.get(opts, :timeout_ms, @default_timeout_ms)
    run_id = Keyword.get(opts, :run_id)
    trace_context = Tracing.current()

    task =
      Task.Supervisor.async_nolink(OpenAgentsRuntime.Tools.TaskSupervisor, fn ->
        register_run_task(run_id)
        :ok = Tracing.put_current(trace_context)
        Tracing.with_phase_span(:tool, %{component: "tool_runner", run_id: run_id}, fun)
      end)

    case Task.yield(task, timeout_ms) || Task.shutdown(task, :brutal_kill) do
      {:ok, value} -> {:ok, value}
      nil -> {:error, :timeout}
      {:exit, _reason} -> {:error, :canceled}
    end
  end

  @spec cancel_run(String.t()) :: :ok
  def cancel_run(run_id) when is_binary(run_id) do
    run_id
    |> task_pids_for_run()
    |> Enum.each(fn pid -> Process.exit(pid, :kill) end)

    :ok
  end

  defp register_run_task(nil), do: :ok

  defp register_run_task(run_id) when is_binary(run_id) do
    Registry.register(OpenAgentsRuntime.ToolTaskRegistry, run_id, :tool_task)
    :ok
  end

  defp task_pids_for_run(run_id) do
    OpenAgentsRuntime.ToolTaskRegistry
    |> Registry.lookup(run_id)
    |> Enum.map(&elem(&1, 0))
    |> Enum.uniq()
  end
end
