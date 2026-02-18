defmodule OpenAgentsRuntime.DS.Compile.PromoteService do
  @moduledoc """
  Artifact promotion/rollback boundary.
  """

  @spec promote(String.t(), String.t()) :: :ok
  def promote(_signature_id, _compiled_id), do: :ok
end
