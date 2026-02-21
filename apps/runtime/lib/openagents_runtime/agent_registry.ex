defmodule OpenAgentsRuntime.AgentRegistry do
  @moduledoc """
  Registry helper functions for runtime agent processes.
  """

  @type agent_id :: String.t()

  @spec via(agent_id()) :: {:via, Registry, {module(), agent_id()}}
  def via(agent_id) when is_binary(agent_id) do
    {:via, Registry, {OpenAgentsRuntime.AgentRegistry, agent_id}}
  end

  @spec whereis(agent_id()) :: pid() | nil
  def whereis(agent_id) when is_binary(agent_id) do
    case Registry.lookup(OpenAgentsRuntime.AgentRegistry, agent_id) do
      [{pid, _value}] -> pid
      [] -> nil
    end
  end
end
