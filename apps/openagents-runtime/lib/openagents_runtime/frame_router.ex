defmodule OpenAgentsRuntime.FrameRouter do
  @moduledoc """
  Entry point for routing incoming frames to runtime processes.
  """

  @spec route(String.t(), String.t()) :: :ok | {:error, term()}
  def route(run_id, frame_id) when is_binary(run_id) and is_binary(frame_id) do
    OpenAgentsRuntime.AgentSupervisor.route_frame(run_id, frame_id)
  end
end
