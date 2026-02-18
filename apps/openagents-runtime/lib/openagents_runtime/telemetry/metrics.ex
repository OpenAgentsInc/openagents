defmodule OpenAgentsRuntime.Telemetry.Metrics do
  @moduledoc """
  Runtime metrics declaration boundary.
  """

  @spec default_prefix() :: String.t()
  def default_prefix, do: "openagents_runtime"
end
