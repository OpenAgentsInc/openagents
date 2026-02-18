defmodule OpenAgentsRuntime.Integrations.AuthTokenVerifierTest do
  use ExUnit.Case, async: false

  alias OpenAgentsRuntime.Integrations.AuthTokenVerifier

  @secret "test-signing-secret"

  setup do
    case :ets.whereis(:openagents_runtime_auth_nonce_cache) do
      :undefined -> :ok
      table -> :ets.delete_all_objects(table)
    end

    :ok
  end

  test "verifies a valid signed token" do
    now = 1_700_000_000

    token =
      signed_token(%{
        "iat" => now,
        "exp" => now + 300,
        "nonce" => "nonce-1",
        "run_id" => "run_123",
        "thread_id" => "thread_abc",
        "user_id" => 42
      })

    assert :ok =
             AuthTokenVerifier.verify(token,
               secret: @secret,
               now: now,
               expected_claims: %{run_id: "run_123", thread_id: "thread_abc", user_id: 42}
             )
  end

  test "rejects missing tokens" do
    assert {:error, :missing_token} = AuthTokenVerifier.verify(nil)
  end

  test "rejects invalid signature" do
    now = 1_700_000_000

    token =
      signed_token(%{
        "iat" => now,
        "exp" => now + 100,
        "nonce" => "nonce-2"
      })

    assert {:error, :invalid_signature} =
             AuthTokenVerifier.verify(token, secret: "wrong-secret", now: now)
  end

  test "rejects expired token" do
    now = 1_700_000_000

    token =
      signed_token(%{
        "iat" => now - 200,
        "exp" => now - 1,
        "nonce" => "nonce-3"
      })

    assert {:error, :token_expired} =
             AuthTokenVerifier.verify(token, secret: @secret, now: now)
  end

  test "rejects future-issued tokens outside allowed clock skew" do
    now = 1_700_000_000

    token =
      signed_token(%{
        "iat" => now + 60,
        "exp" => now + 120,
        "nonce" => "nonce-4"
      })

    assert {:error, :token_not_yet_valid} =
             AuthTokenVerifier.verify(token, secret: @secret, now: now)
  end

  test "rejects claim mismatch" do
    now = 1_700_000_000

    token =
      signed_token(%{
        "iat" => now,
        "exp" => now + 100,
        "nonce" => "nonce-5",
        "run_id" => "run_123"
      })

    assert {:error, {:claim_mismatch, :run_id}} =
             AuthTokenVerifier.verify(token,
               secret: @secret,
               now: now,
               expected_claims: %{run_id: "run_999"}
             )
  end

  test "rejects replayed nonce" do
    now = 1_700_000_000

    token =
      signed_token(%{
        "iat" => now,
        "exp" => now + 100,
        "nonce" => "nonce-6"
      })

    assert :ok = AuthTokenVerifier.verify(token, secret: @secret, now: now)
    assert {:error, :token_replayed} = AuthTokenVerifier.verify(token, secret: @secret, now: now)
  end

  test "returns standardized error details" do
    assert %{code: "unauthorized", message: "invalid token signature"} =
             AuthTokenVerifier.error_details(:invalid_signature)

    assert %{code: "forbidden", message: "claim mismatch for run_id"} =
             AuthTokenVerifier.error_details({:claim_mismatch, :run_id})
  end

  defp signed_token(claims) do
    payload = Jason.encode!(claims)
    payload_segment = Base.url_encode64(payload, padding: false)
    signature = :crypto.mac(:hmac, :sha256, @secret, payload_segment)
    signature_segment = Base.url_encode64(signature, padding: false)

    "v1.#{payload_segment}.#{signature_segment}"
  end
end
