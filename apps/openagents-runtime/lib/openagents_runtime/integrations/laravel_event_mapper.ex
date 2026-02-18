defmodule OpenAgentsRuntime.Integrations.LaravelEventMapper do
  @moduledoc """
  Maps runtime-native events to the current Laravel SSE contract.
  """

  @spec map_event(atom(), map()) :: map()
  def map_event(:run_started, payload), do: Map.merge(%{type: "start"}, payload)
  def map_event(:run_finished, payload), do: Map.merge(%{type: "finish"}, payload)
  def map_event(:text_delta, payload), do: Map.merge(%{type: "text-delta"}, payload)
  def map_event(_other, payload), do: payload
end
