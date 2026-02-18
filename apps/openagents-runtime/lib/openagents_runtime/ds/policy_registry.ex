defmodule OpenAgentsRuntime.DS.PolicyRegistry do
  @moduledoc """
  Active artifact pointer lookup boundary.
  """

  @spec active_artifact(String.t()) :: {:ok, nil}
  def active_artifact(signature_id) when is_binary(signature_id), do: {:ok, nil}
end
