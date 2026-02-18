defmodule OpenAgentsRuntime.FrameCompactor do
  @moduledoc """
  Placeholder boundary for compaction orchestration.
  """

  @spec compact([map()]) :: map()
  def compact(frames) when is_list(frames) do
    %{frame_count: length(frames)}
  end
end
