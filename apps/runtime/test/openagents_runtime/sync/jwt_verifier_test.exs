defmodule OpenAgentsRuntime.Sync.JwtVerifierTest do
  use OpenAgentsRuntime.DataCase, async: true

  import OpenAgentsRuntimeWeb.AuthHelpers

  alias OpenAgentsRuntime.Sync.JwtVerifier

  @run_topic "runtime.run_summaries"

  test "verify_and_claims accepts valid HS256 sync jwt" do
    token = valid_sync_jwt(oa_sync_scopes: [@run_topic], oa_org_id: "org_123")

    assert {:ok, claims} = JwtVerifier.verify_and_claims(token)
    assert claims["oa_org_id"] == "org_123"
    assert claims["oa_sync_scopes"] == [@run_topic]
  end

  test "verify_and_claims rejects unknown kid" do
    token =
      valid_sync_jwt(
        kid: "sync-auth-unknown-v3",
        key: "sync-unknown-signing-key",
        oa_sync_scopes: [@run_topic]
      )

    assert {:error, :unknown_kid} = JwtVerifier.verify_and_claims(token)
  end

  test "verify_and_claims rejects claim mismatch" do
    token =
      valid_sync_jwt(
        audience: "wrong-audience",
        oa_sync_scopes: [@run_topic]
      )

    assert {:error, {:claim_mismatch, :aud}} = JwtVerifier.verify_and_claims(token)
  end

  @tag :chaos_drill
  test "verify_and_claims rejects expired token" do
    now = System.system_time(:second)
    token = valid_sync_jwt(now: now - 600, ttl_seconds: 300, oa_sync_scopes: [@run_topic])

    assert {:error, :token_expired} = JwtVerifier.verify_and_claims(token, now: now)
  end

  test "verify_and_claims accepts rotated kid from configured keyring" do
    old_sync_auth = Application.get_env(:openagents_runtime, :khala_sync_auth, [])

    Application.put_env(:openagents_runtime, :khala_sync_auth,
      issuer: "https://openagents.test",
      audience: "openagents-sync-test",
      claims_version: "oa_sync_claims_v1",
      allowed_algs: ["HS256"],
      hs256_keys: %{
        "sync-auth-test-v1" => "sync-test-signing-key",
        "sync-auth-rotated-v2" => "sync-rotated-signing-key"
      }
    )

    on_exit(fn ->
      Application.put_env(:openagents_runtime, :khala_sync_auth, old_sync_auth)
    end)

    token =
      valid_sync_jwt(
        kid: "sync-auth-rotated-v2",
        key: "sync-rotated-signing-key",
        oa_sync_scopes: [@run_topic]
      )

    assert {:ok, claims} = JwtVerifier.verify_and_claims(token)
    assert claims["oa_sync_scopes"] == [@run_topic]
  end
end
