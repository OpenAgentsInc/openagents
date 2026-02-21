defmodule OpenAgentsRuntime.Contracts.RuntimeOrchestrationProtoContractTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Contracts.RuntimeOrchestrationProtoContract

  test "runtime orchestration proto + fixture contract check passes" do
    assert :ok = RuntimeOrchestrationProtoContract.check()
  end
end
