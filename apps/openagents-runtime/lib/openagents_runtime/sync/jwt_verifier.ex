defmodule OpenAgentsRuntime.Sync.JwtVerifier do
  @moduledoc """
  Verifies Khala sync JWTs (HS256 today) with `kid` key rotation support.

  The verifier is intentionally keyring-driven so HS256 can evolve to RS256/JWKS
  without changing socket call sites.
  """

  @clock_skew_seconds 30

  @type verify_error ::
          :missing_token
          | :invalid_token_format
          | :invalid_token_encoding
          | :invalid_header
          | :unsupported_alg
          | :missing_kid
          | :unknown_kid
          | :invalid_signature
          | :invalid_claims
          | :token_expired
          | :token_not_yet_valid
          | {:claim_mismatch, atom()}

  @type verify_opt ::
          {:expected_claims, map()}
          | {:keyring, %{optional(String.t()) => String.t()}}
          | {:allowed_algs, [String.t()]}
          | {:now, non_neg_integer()}

  @spec verify(binary() | nil, [verify_opt()]) :: :ok | {:error, verify_error()}
  def verify(nil, _opts), do: {:error, :missing_token}
  def verify(token, _opts) when not is_binary(token), do: {:error, :invalid_token_format}

  def verify(token, opts) when is_binary(token) do
    case verify_and_claims(token, opts) do
      {:ok, _claims} -> :ok
      {:error, reason} -> {:error, reason}
    end
  end

  @spec verify_and_claims(binary() | nil, [verify_opt()]) ::
          {:ok, map()} | {:error, verify_error()}
  def verify_and_claims(nil, _opts), do: {:error, :missing_token}

  def verify_and_claims(token, _opts) when not is_binary(token),
    do: {:error, :invalid_token_format}

  def verify_and_claims(token, opts) when is_binary(token) do
    now = opts[:now] || System.system_time(:second)

    with {:ok,
          %{header: header, claims: claims, signing_input: signing_input, signature: signature}} <-
           decode_token(token),
         :ok <- validate_header(header, opts),
         :ok <- validate_signature(header, signing_input, signature, opts),
         :ok <- validate_claims(claims, now),
         :ok <- validate_expected_claims(claims, opts) do
      {:ok, claims}
    end
  end

  @spec verify_and_claims(binary() | nil) :: {:ok, map()} | {:error, verify_error()}
  def verify_and_claims(token), do: verify_and_claims(token, [])

  @spec verify(binary() | nil) :: :ok | {:error, verify_error()}
  def verify(token), do: verify(token, [])

  defp decode_token(token) do
    with [header_segment, payload_segment, signature_segment] <-
           String.split(token, ".", parts: 3),
         {:ok, header_json} <- b64_decode(header_segment),
         {:ok, payload_json} <- b64_decode(payload_segment),
         {:ok, header} <- Jason.decode(header_json),
         {:ok, claims} <- Jason.decode(payload_json),
         true <- is_map(header) and is_map(claims),
         {:ok, signature} <- b64_decode(signature_segment) do
      {:ok,
       %{
         header: header,
         claims: claims,
         signing_input: header_segment <> "." <> payload_segment,
         signature: signature
       }}
    else
      false -> {:error, :invalid_token_encoding}
      _ -> {:error, :invalid_token_encoding}
    end
  rescue
    _ -> {:error, :invalid_token_format}
  end

  defp validate_header(header, opts) when is_map(header) do
    alg = Map.get(header, "alg")

    allowed_algs =
      Keyword.get(opts, :allowed_algs) ||
        Application.get_env(:openagents_runtime, :khala_sync_auth, [])
        |> Keyword.get(:allowed_algs, ["HS256"])

    cond do
      not is_binary(alg) or alg == "" ->
        {:error, :invalid_header}

      alg not in allowed_algs ->
        {:error, :unsupported_alg}

      alg == "HS256" and missing_kid?(header) ->
        {:error, :missing_kid}

      true ->
        :ok
    end
  end

  defp validate_header(_header, _opts), do: {:error, :invalid_header}

  defp validate_signature(%{"alg" => "HS256", "kid" => kid}, signing_input, signature, opts) do
    keyring =
      Keyword.get(opts, :keyring) ||
        Application.get_env(:openagents_runtime, :khala_sync_auth, [])
        |> Keyword.get(:hs256_keys, %{})

    case keyring do
      %{} ->
        case Map.get(keyring, kid) do
          key when is_binary(key) and key != "" ->
            expected = :crypto.mac(:hmac, :sha256, key, signing_input)

            if Plug.Crypto.secure_compare(expected, signature) do
              :ok
            else
              {:error, :invalid_signature}
            end

          _other ->
            {:error, :unknown_kid}
        end

      _other ->
        {:error, :unknown_kid}
    end
  end

  defp validate_signature(_header, _signing_input, _signature, _opts),
    do: {:error, :unsupported_alg}

  defp validate_claims(claims, now) when is_map(claims) do
    with {:ok, iat} <- claim_int(claims, "iat"),
         {:ok, exp} <- claim_int(claims, "exp"),
         {:ok, nbf} <- claim_int_or_default(claims, "nbf", iat),
         {:ok, sub} <- claim_string(claims, "sub"),
         {:ok, jti} <- claim_string(claims, "jti"),
         {:ok, org_id} <- claim_string(claims, "oa_org_id"),
         {:ok, scopes} <- claim_scopes(claims) do
      cond do
        sub == "" or jti == "" or org_id == "" ->
          {:error, :invalid_claims}

        scopes == [] ->
          {:error, :invalid_claims}

        exp <= now ->
          {:error, :token_expired}

        iat > now + @clock_skew_seconds or nbf > now + @clock_skew_seconds ->
          {:error, :token_not_yet_valid}

        exp <= iat ->
          {:error, :invalid_claims}

        true ->
          :ok
      end
    else
      _ -> {:error, :invalid_claims}
    end
  end

  defp validate_claims(_claims, _now), do: {:error, :invalid_claims}

  defp validate_expected_claims(claims, opts) do
    expected_claims =
      Keyword.get(opts, :expected_claims) ||
        default_expected_claims_from_config()

    Enum.reduce_while(expected_claims, :ok, fn {key, expected_value}, _acc ->
      claim_key = to_string(key)
      actual_value = Map.get(claims, claim_key)

      if claim_matches?(actual_value, expected_value) do
        {:cont, :ok}
      else
        {:halt, {:error, {:claim_mismatch, key}}}
      end
    end)
  end

  defp default_expected_claims_from_config do
    config = Application.get_env(:openagents_runtime, :khala_sync_auth, [])

    %{}
    |> maybe_put_expected_claim(:iss, Keyword.get(config, :issuer))
    |> maybe_put_expected_claim(:aud, Keyword.get(config, :audience))
    |> maybe_put_expected_claim(:oa_claims_version, Keyword.get(config, :claims_version))
  end

  defp maybe_put_expected_claim(claims, _key, nil), do: claims
  defp maybe_put_expected_claim(claims, _key, ""), do: claims
  defp maybe_put_expected_claim(claims, key, value), do: Map.put(claims, key, value)

  defp claim_matches?(actual, expected) when is_list(actual), do: expected in actual
  defp claim_matches?(actual, expected), do: actual == expected

  defp missing_kid?(header) do
    case Map.get(header, "kid") do
      value when is_binary(value) and value != "" -> false
      _other -> true
    end
  end

  defp claim_int(claims, key) do
    case Map.get(claims, key) do
      value when is_integer(value) -> {:ok, value}
      _ -> :error
    end
  end

  defp claim_int_or_default(claims, key, default) do
    case Map.get(claims, key) do
      nil -> {:ok, default}
      value when is_integer(value) -> {:ok, value}
      _ -> :error
    end
  end

  defp claim_string(claims, key) do
    case Map.get(claims, key) do
      value when is_binary(value) -> {:ok, value}
      _ -> :error
    end
  end

  defp claim_scopes(claims) do
    case Map.get(claims, "oa_sync_scopes") do
      scopes when is_list(scopes) ->
        normalized =
          scopes
          |> Enum.filter(&is_binary/1)
          |> Enum.map(&String.trim/1)
          |> Enum.reject(&(&1 == ""))

        {:ok, normalized}

      scopes when is_binary(scopes) ->
        scopes
        |> String.split(",")
        |> Enum.map(&String.trim/1)
        |> Enum.reject(&(&1 == ""))
        |> then(&{:ok, &1})

      _other ->
        :error
    end
  end

  defp b64_decode(segment), do: Base.url_decode64(segment, padding: false)
end
