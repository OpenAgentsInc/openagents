defmodule OpenAgentsRuntime.Contracts.ControlPlaneAuthProtoContractTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Contracts.ControlPlaneAuthProtoContract

  test "control-plane auth proto and fixture remain converged" do
    assert :ok = ControlPlaneAuthProtoContract.check()
  end
end
