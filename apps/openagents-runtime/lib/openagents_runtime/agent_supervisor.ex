defmodule OpenAgentsRuntime.AgentSupervisor do
  @moduledoc """
  Helper API for starting and supervising per-run agent processes.
  """

  @spec start_agent(String.t()) :: DynamicSupervisor.on_start_child()
  def start_agent(run_id) when is_binary(run_id) do
    spec = {OpenAgentsRuntime.AgentProcess, run_id}
    DynamicSupervisor.start_child(OpenAgentsRuntime.AgentSupervisor, spec)
  end
end
