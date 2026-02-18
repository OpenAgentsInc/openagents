defmodule OpenAgentsRuntime.DS.ToolReplay do
  @moduledoc """
  Bounded tool replay summary builder for context reinjection.
  """

  @spec summarize([map()]) :: String.t()
  def summarize(events) when is_list(events) do
    "events=" <> Integer.to_string(length(events))
  end
end
