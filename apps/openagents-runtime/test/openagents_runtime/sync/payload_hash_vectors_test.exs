defmodule OpenAgentsRuntime.Sync.PayloadHashVectorsTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Sync.PayloadHash

  test "canonical json and hash vectors remain stable across fixture set" do
    fixture =
      fixture_path()
      |> File.read!()
      |> Jason.decode!()

    assert fixture["version"] == "khala.payload_hash.v1"
    assert fixture["algorithm"] == "sha256"

    Enum.each(fixture["vectors"], fn vector ->
      payload = vector["payload"]
      expected_canonical_json = vector["canonical_json"]
      expected_hash = vector["sha256"]

      assert PayloadHash.canonical_json(payload) == expected_canonical_json,
             "canonical JSON mismatch for #{vector["name"]}"

      assert PayloadHash.sha256_canonical_json(payload) == expected_hash,
             "hash mismatch for #{vector["name"]}"
    end)
  end

  defp fixture_path do
    Path.expand(
      "../../../../../docs/protocol/testdata/khala_payload_hash_vectors.v1.json",
      __DIR__
    )
  end
end
