defmodule OpenAgentsRuntime.DS.Signatures.CatalogTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.DS.Signatures.Catalog

  test "exposes stable signature ids and fetch returns definitions" do
    ids = Catalog.signature_ids()
    assert length(ids) >= 3
    assert "@openagents/autopilot/blueprint/SelectTool.v1" in ids

    assert {:ok, signature} = Catalog.fetch("@openagents/autopilot/blueprint/SelectTool.v1")
    assert signature.signature_id == "@openagents/autopilot/blueprint/SelectTool.v1"
    assert signature.namespace == "@openagents/autopilot/blueprint"
    assert signature.version == 1
  end

  test "stable_signature_id and hashes are deterministic" do
    assert Catalog.stable_signature_id("@openagents/autopilot/test", "MySig", 2) ==
             "@openagents/autopilot/test/MySig.v2"

    signature_id = "@openagents/autopilot/canary/RecapThread.v1"

    assert {:ok, hashes_a} = Catalog.hashes(signature_id)
    assert {:ok, hashes_b} = Catalog.hashes(signature_id)

    assert hashes_a == hashes_b
    assert String.length(hashes_a.schema_hash) == 64
    assert String.length(hashes_a.prompt_hash) == 64
    assert String.length(hashes_a.program_hash) == 64
  end

  test "fingerprint contains signature and hash anchors" do
    signature_id = "@openagents/autopilot/rlm/SummarizeThread.v1"

    assert {:ok, fingerprint} = Catalog.fingerprint(signature_id)

    assert fingerprint.signature_id == signature_id
    assert is_binary(fingerprint.schema_hash)
    assert is_binary(fingerprint.prompt_hash)
    assert is_binary(fingerprint.program_hash)
    assert fingerprint.catalog_version == Catalog.catalog_version()
  end

  test "artifact compatibility validation detects missing and mismatched hashes" do
    signature_id = "@openagents/autopilot/blueprint/SelectTool.v1"
    assert {:ok, hashes} = Catalog.hashes(signature_id)

    valid_artifact = %{
      schema_hash: hashes.schema_hash,
      prompt_hash: hashes.prompt_hash,
      program_hash: hashes.program_hash
    }

    assert :ok = Catalog.validate_artifact(signature_id, valid_artifact)
    assert Catalog.artifact_compatible?(signature_id, valid_artifact)

    assert {:error, {:missing_hash, :program_hash}} =
             Catalog.validate_artifact(signature_id, Map.delete(valid_artifact, :program_hash))

    mismatch = %{valid_artifact | prompt_hash: String.duplicate("a", 64)}

    assert {:error, {:hash_mismatch, :prompt_hash}} =
             Catalog.validate_artifact(signature_id, mismatch)

    refute Catalog.artifact_compatible?(signature_id, mismatch)
  end

  test "returns not found for unknown signatures" do
    assert {:error, :not_found} = Catalog.fetch("missing.v1")
    assert {:error, :signature_not_found} = Catalog.validate_artifact("missing.v1", %{})
  end
end
