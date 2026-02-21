defmodule OpenAgentsRuntime.AgentSupervisor do
  @moduledoc """
  Helper API for starting and supervising per-run agent processes.
  """

  alias OpenAgentsRuntime.AgentRegistry
  alias OpenAgentsRuntime.AgentProcess

  @type start_result :: {:ok, pid()} | {:error, term()}

  @spec start_agent(String.t()) :: DynamicSupervisor.on_start_child()
  def start_agent(run_id) when is_binary(run_id) do
    spec = {AgentProcess, run_id}
    DynamicSupervisor.start_child(OpenAgentsRuntime.AgentSupervisor, spec)
  end

  @spec ensure_agent(String.t()) :: start_result()
  def ensure_agent(run_id) when is_binary(run_id) do
    case AgentRegistry.whereis(run_id) do
      pid when is_pid(pid) ->
        {:ok, pid}

      nil ->
        case start_agent(run_id) do
          {:ok, pid} -> {:ok, pid}
          {:error, {:already_started, pid}} -> {:ok, pid}
          {:error, {:already_present, _child_id}} -> ensure_agent(run_id)
          {:error, reason} -> {:error, reason}
          other -> other
        end
    end
  end

  @spec route_frame(String.t(), String.t()) :: :ok | {:error, term()}
  def route_frame(run_id, frame_id) when is_binary(run_id) and is_binary(frame_id) do
    with {:ok, _pid} <- ensure_agent(run_id),
         :ok <- AgentProcess.ingest_frame(run_id, frame_id) do
      :ok
    end
  end
end
