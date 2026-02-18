defmodule OpenAgentsRuntime.DS.Signatures.Catalog do
  @moduledoc """
  Stable DS signature ids for runtime-level policy decisions.
  """

  @spec signature_ids() :: [String.t()]
  def signature_ids do
    [
      "@openagents/autopilot/blueprint/SelectTool.v1",
      "@openagents/autopilot/canary/RecapThread.v1",
      "@openagents/autopilot/rlm/SummarizeThread.v1"
    ]
  end
end
