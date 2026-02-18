defmodule OpenAgentsRuntime.Runs.RunEvents do
  @moduledoc """
  Event helpers used by stream/log layers.
  """

  @type run_event :: %{required(:run_id) => String.t(), required(:seq) => non_neg_integer()}

  @spec sort([run_event()]) :: [run_event()]
  def sort(events) when is_list(events) do
    Enum.sort_by(events, &{&1.run_id, &1.seq})
  end
end
