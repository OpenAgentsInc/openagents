defmodule Mix.Tasks.Runtime.Contract.Check do
  @shortdoc "Validates runtime internal API contract artifacts and route convergence"

  use Mix.Task

  alias OpenAgentsRuntime.Contracts.Layer0ProtoContract
  alias OpenAgentsRuntime.Contracts.InternalAPIContract

  @impl true
  def run(_args) do
    checks = [
      {"internal API contract", &InternalAPIContract.check/0},
      {"layer-0 proto contract", &Layer0ProtoContract.check/0}
    ]

    errors =
      Enum.reduce(checks, [], fn {label, check_fn}, acc ->
        case check_fn.() do
          :ok ->
            acc

          {:error, check_errors} ->
            prefixed = Enum.map(check_errors, &"[#{label}] #{&1}")
            prefixed ++ acc
        end
      end)

    if errors == [] do
      Mix.shell().info("runtime contract check passed")
    else
      message =
        errors
        |> Enum.reverse()
        |> Enum.map_join("\n", &"  - #{&1}")

      Mix.raise("runtime contract check failed:\n#{message}")
    end
  end
end
