defmodule OpenAgentsRuntime.Tools.Comms.ProviderAdapter do
  @moduledoc """
  Provider adapter behaviour for comms tool-pack send execution.
  """

  @callback send(request :: map(), manifest :: map(), opts :: keyword()) ::
              {:ok, map()} | {:error, map() | term()}
end
