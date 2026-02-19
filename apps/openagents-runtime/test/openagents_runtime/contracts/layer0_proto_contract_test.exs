defmodule OpenAgentsRuntime.Contracts.Layer0ProtoContractTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Contracts.Layer0ProtoContract

  test "layer-0 proto artifacts and runtime mappings are converged" do
    assert :ok = Layer0ProtoContract.check()
  end
end
