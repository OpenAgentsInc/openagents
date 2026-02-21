defmodule OpenAgentsRuntime.Contracts.CodexProtoContractTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Contracts.CodexProtoContract

  test "codex proto and fixture remain converged" do
    assert :ok = CodexProtoContract.check()
  end
end
