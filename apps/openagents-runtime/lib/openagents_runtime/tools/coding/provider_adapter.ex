defmodule OpenAgentsRuntime.Tools.Coding.ProviderAdapter do
  @moduledoc """
  Provider adapter behaviour for coding tool-pack operations.
  """

  @callback execute(request :: map(), manifest :: map(), opts :: keyword()) ::
              {:ok, map()} | {:error, map() | term()}
end
