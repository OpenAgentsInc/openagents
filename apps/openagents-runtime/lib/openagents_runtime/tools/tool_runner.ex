defmodule OpenAgentsRuntime.Tools.ToolRunner do
  @moduledoc """
  Executes tool work inside a supervised task boundary.
  """

  @spec run((-> term()), timeout()) :: {:ok, term()} | {:error, :timeout}
  def run(fun, timeout_ms \\ 5_000) when is_function(fun, 0) and is_integer(timeout_ms) do
    task = Task.Supervisor.async_nolink(OpenAgentsRuntime.Tools.TaskSupervisor, fun)

    case Task.yield(task, timeout_ms) || Task.shutdown(task) do
      {:ok, value} -> {:ok, value}
      nil -> {:error, :timeout}
    end
  end
end
