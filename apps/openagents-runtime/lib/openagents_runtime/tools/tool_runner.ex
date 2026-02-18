defmodule OpenAgentsRuntime.Tools.ToolRunner do
  @moduledoc """
  Executes tool work inside a supervised task boundary.
  """

  alias OpenAgentsRuntime.Telemetry.Tracing

  @spec run((-> term()), timeout()) :: {:ok, term()} | {:error, :timeout}
  def run(fun, timeout_ms \\ 5_000) when is_function(fun, 0) and is_integer(timeout_ms) do
    trace_context = Tracing.current()

    task =
      Task.Supervisor.async_nolink(OpenAgentsRuntime.Tools.TaskSupervisor, fn ->
        :ok = Tracing.put_current(trace_context)
        Tracing.with_phase_span(:tool, %{component: "tool_runner"}, fun)
      end)

    case Task.yield(task, timeout_ms) || Task.shutdown(task) do
      {:ok, value} -> {:ok, value}
      nil -> {:error, :timeout}
    end
  end
end
