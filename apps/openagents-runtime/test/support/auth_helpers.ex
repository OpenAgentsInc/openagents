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
