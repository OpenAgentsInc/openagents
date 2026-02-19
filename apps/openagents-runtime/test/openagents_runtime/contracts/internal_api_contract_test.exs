defmodule OpenAgentsRuntime.Contracts.InternalAPIContractTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Contracts.InternalAPIContract

  test "internal API artifacts and routes are converged" do
    assert :ok = InternalAPIContract.check()
  end
end
