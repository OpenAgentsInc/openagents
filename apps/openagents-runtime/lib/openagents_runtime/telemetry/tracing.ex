defmodule OpenAgentsRuntime.Telemetry.Tracing do
  @moduledoc """
  Runtime tracing helper boundary.
  """

  @spec trace_id_header() :: String.t()
  def trace_id_header, do: "traceparent"
end
