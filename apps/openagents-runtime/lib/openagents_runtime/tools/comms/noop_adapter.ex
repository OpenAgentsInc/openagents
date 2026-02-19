defmodule OpenAgentsRuntime.Tools.Comms.NoopAdapter do
  @moduledoc """
  Deterministic no-op comms adapter for non-production runtime paths and tests.
  """

  @behaviour OpenAgentsRuntime.Tools.Comms.ProviderAdapter

  alias OpenAgentsRuntime.DS.Receipts

  @impl true
  def send(request, _manifest, _opts) when is_map(request) do
    message_id =
      request
      |> Map.take(["integration_id", "recipient", "template_id", "variables"])
      |> Receipts.stable_hash()
      |> String.slice(0, 24)
      |> then(&"noop_#{&1}")

    {:ok, %{"message_id" => message_id, "state" => "sent"}}
  end
end
