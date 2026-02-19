defmodule Mix.Tasks.Runtime.Contract.Check do
  @shortdoc "Validates runtime internal API contract artifacts and route convergence"

  use Mix.Task

  alias OpenAgentsRuntime.Contracts.InternalAPIContract

  @impl true
  def run(_args) do
    case InternalAPIContract.check() do
      :ok ->
        Mix.shell().info("runtime contract check passed")

      {:error, errors} ->
        message =
          errors
          |> Enum.reverse()
          |> Enum.map_join("\n", &"  - #{&1}")

        Mix.raise("runtime contract check failed:\n#{message}")
    end
  end
end
