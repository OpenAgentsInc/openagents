defmodule OpenAgentsRuntime.DS.Traces do
  @moduledoc """
  Trace pointer helper for large trace payloads.
  """

  @spec pointer(String.t(), String.t()) :: String.t()
  def pointer(run_id, trace_id) when is_binary(run_id) and is_binary(trace_id) do
    "trace:" <> run_id <> ":" <> trace_id
  end
end
