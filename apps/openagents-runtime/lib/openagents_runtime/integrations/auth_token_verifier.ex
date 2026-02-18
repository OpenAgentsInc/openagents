defmodule OpenAgentsRuntime.Integrations.AuthTokenVerifier do
  @moduledoc """
  Boundary for internal token verification.
  """

  @spec verify(binary() | nil) :: :ok | {:error, :missing_token}
  def verify(nil), do: {:error, :missing_token}
  def verify(token) when is_binary(token) and byte_size(token) > 0, do: :ok
end
