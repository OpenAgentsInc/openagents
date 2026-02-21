defmodule OpenAgentsRuntimeWeb.AuthHelpers do
  @moduledoc false

  import Plug.Conn

  @spec valid_signature_token(keyword()) :: String.t()
  def valid_signature_token(opts \\ []) do
    secret = Application.get_env(:openagents_runtime, :runtime_signature_secret)
    now = Keyword.get(opts, :now, System.system_time(:second))

    claims =
      %{
        "iat" => now,
        "exp" => now + 300,
        "nonce" => "nonce-#{System.unique_integer([:positive])}"
      }
      |> maybe_put_claim("run_id", opts[:run_id])
      |> maybe_put_claim("thread_id", opts[:thread_id])
      |> maybe_put_claim("user_id", opts[:user_id])
      |> maybe_put_claim("guest_scope", opts[:guest_scope])
      |> maybe_put_claim("oa_org_id", opts[:oa_org_id])
      |> maybe_put_claim("oa_sync_scopes", opts[:oa_sync_scopes])
      |> maybe_merge_claims(opts[:extra_claims])

    payload_segment = claims |> Jason.encode!() |> Base.url_encode64(padding: false)
    signature = :crypto.mac(:hmac, :sha256, secret, payload_segment)
    signature_segment = Base.url_encode64(signature, padding: false)

    "v1.#{payload_segment}.#{signature_segment}"
  end

  @spec valid_sync_jwt(keyword()) :: String.t()
  def valid_sync_jwt(opts \\ []) do
    now = Keyword.get(opts, :now, System.system_time(:second))
    ttl_seconds = Keyword.get(opts, :ttl_seconds, 300)

    sync_auth_config = Application.get_env(:openagents_runtime, :khala_sync_auth, [])

    kid =
      Keyword.get(
        opts,
        :kid,
        sync_auth_config
        |> Keyword.get(:hs256_keys, %{})
        |> Map.keys()
        |> List.first() ||
          "sync-auth-v1"
      )

    keyring =
      Keyword.get(opts, :keyring) ||
        sync_auth_config
        |> Keyword.get(:hs256_keys, %{})

    signing_key =
      Keyword.get_lazy(opts, :key, fn ->
        case keyring do
          %{} -> Map.get(keyring, kid)
          _other -> nil
        end
      end)

    if not is_binary(signing_key) or signing_key == "" do
      raise ArgumentError,
            "valid_sync_jwt requires a signing key (via :key or :keyring/:khala_sync_auth)"
    end

    issuer =
      Keyword.get(
        opts,
        :issuer,
        sync_auth_config
        |> Keyword.get(:issuer, "https://openagents.test")
      )

    audience =
      Keyword.get(
        opts,
        :audience,
        sync_auth_config
        |> Keyword.get(:audience, "openagents-sync")
      )

    claims_version =
      Keyword.get(
        opts,
        :claims_version,
        sync_auth_config
        |> Keyword.get(:claims_version, "oa_sync_claims_v1")
      )

    claims =
      %{
        "iss" => issuer,
        "aud" => audience,
        "sub" => "user:#{opts[:user_id] || 1}",
        "iat" => now,
        "nbf" => now,
        "exp" => now + ttl_seconds,
        "jti" => "jti-#{System.unique_integer([:positive])}",
        "oa_user_id" => opts[:user_id] || 1,
        "oa_org_id" => opts[:oa_org_id] || "org_123",
        "oa_session_id" =>
          opts[:oa_session_id] || "sess_test_#{System.unique_integer([:positive])}",
        "oa_device_id" => opts[:oa_device_id] || "device:test",
        "oa_sync_scopes" => opts[:oa_sync_scopes] || [],
        "oa_claims_version" => claims_version
      }
      |> maybe_merge_claims(opts[:extra_claims])

    header = %{
      "alg" => "HS256",
      "typ" => "JWT",
      "kid" => kid
    }

    header_segment = header |> Jason.encode!() |> Base.url_encode64(padding: false)
    payload_segment = claims |> Jason.encode!() |> Base.url_encode64(padding: false)
    signing_input = "#{header_segment}.#{payload_segment}"
    signature = :crypto.mac(:hmac, :sha256, signing_key, signing_input)
    signature_segment = Base.url_encode64(signature, padding: false)

    "#{signing_input}.#{signature_segment}"
  end

  @spec put_internal_auth(Plug.Conn.t(), keyword()) :: Plug.Conn.t()
  def put_internal_auth(conn, opts \\ []) do
    token = Keyword.get_lazy(opts, :token, fn -> valid_signature_token(opts) end)

    conn
    |> put_req_header("x-oa-runtime-signature", token)
    |> maybe_put_header("x-oa-user-id", opts[:user_id])
    |> maybe_put_header("x-oa-guest-scope", opts[:guest_scope])
  end

  defp maybe_put_claim(claims, _key, nil), do: claims
  defp maybe_put_claim(claims, key, value), do: Map.put(claims, key, value)
  defp maybe_merge_claims(claims, extra) when is_map(extra), do: Map.merge(claims, extra)
  defp maybe_merge_claims(claims, _extra), do: claims

  defp maybe_put_header(conn, _key, nil), do: conn
  defp maybe_put_header(conn, key, value), do: put_req_header(conn, key, to_string(value))
end
