defmodule OpenAgentsRuntime.Integrations.AuthTokenVerifier do
  @moduledoc """
  Verifies signed internal runtime tokens carried in `X-OA-RUNTIME-SIGNATURE`.

  Token format:

      v1.<base64url(payload_json)>.<base64url(hmac_sha256(payload_segment, secret))>

  Required payload claims:

  - `iat` (issued-at unix seconds)
  - `exp` (expiry unix seconds)
  - `nonce` (single-use replay guard)
  """

  @clock_skew_seconds 30
  @replay_table :openagents_runtime_auth_nonce_cache

  @type verify_error ::
          :missing_token
          | :invalid_token_format
          | :invalid_token_encoding
          | :invalid_signature
          | :invalid_claims
          | :token_expired
          | :token_not_yet_valid
          | :token_replayed
          | {:claim_mismatch, atom()}

  @type verify_opt ::
          {:expected_claims, map()}
          | {:secret, binary()}
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
    with {:ok, %{payload_segment: payload_segment, payload: claims, signature: signature}} <-
           decode_token(token),
         :ok <- validate_signature(payload_segment, signature, opts),
         :ok <- validate_claims(claims, opts),
         :ok <- validate_expected_claims(claims, opts),
         :ok <- guard_replay(claims, opts[:now] || System.system_time(:second)) do
      {:ok, claims}
    end
  end

  @spec verify(binary() | nil) :: :ok | {:error, verify_error()}
  def verify(token), do: verify(token, [])

  @spec error_details(verify_error()) :: %{code: String.t(), message: String.t()}
  def error_details(:missing_token),
    do: %{code: "unauthorized", message: "missing runtime signature"}

  def error_details(:invalid_token_format),
    do: %{code: "unauthorized", message: "invalid token format"}

  def error_details(:invalid_token_encoding),
    do: %{code: "unauthorized", message: "invalid token encoding"}

  def error_details(:invalid_signature),
    do: %{code: "unauthorized", message: "invalid token signature"}

  def error_details(:invalid_claims), do: %{code: "unauthorized", message: "invalid token claims"}
  def error_details(:token_expired), do: %{code: "unauthorized", message: "token expired"}

  def error_details(:token_not_yet_valid),
    do: %{code: "unauthorized", message: "token issued in the future"}

  def error_details(:token_replayed),
    do: %{code: "unauthorized", message: "token nonce already used"}

  def error_details({:claim_mismatch, claim}),
    do: %{code: "forbidden", message: "claim mismatch for #{claim}"}

  defp decode_token(token) do
    with ["v1", payload_segment, signature_segment] <- String.split(token, ".", parts: 3),
         {:ok, payload_json} <- b64_decode(payload_segment),
         {:ok, claims} <- Jason.decode(payload_json),
         {:ok, signature} <- b64_decode(signature_segment) do
      {:ok,
       %{
         payload_segment: payload_segment,
         payload: claims,
         signature: signature
       }}
    else
      _ -> {:error, :invalid_token_encoding}
    end
  rescue
    _ -> {:error, :invalid_token_format}
  end

  defp validate_signature(payload_segment, given_signature, opts) do
    secret = secret(opts)
    expected = :crypto.mac(:hmac, :sha256, secret, payload_segment)

    if Plug.Crypto.secure_compare(given_signature, expected) do
      :ok
    else
      {:error, :invalid_signature}
    end
  end

  defp validate_claims(claims, opts) when is_map(claims) do
    now = opts[:now] || System.system_time(:second)

    with {:ok, iat} <- claim_int(claims, "iat"),
         {:ok, exp} <- claim_int(claims, "exp"),
         {:ok, nonce} <- claim_string(claims, "nonce"),
         true <- nonce != "" do
      cond do
        exp <= now ->
          {:error, :token_expired}

        iat > now + @clock_skew_seconds ->
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

  defp validate_claims(_claims, _opts), do: {:error, :invalid_claims}

  defp validate_expected_claims(claims, opts) do
    expected_claims = Keyword.get(opts, :expected_claims, %{})

    Enum.reduce_while(expected_claims, :ok, fn {key, expected_value}, _acc ->
      claim_key = to_string(key)

      if Map.get(claims, claim_key) == expected_value do
        {:cont, :ok}
      else
        {:halt, {:error, {:claim_mismatch, key}}}
      end
    end)
  end

  defp guard_replay(claims, now) do
    nonce = Map.get(claims, "nonce")
    exp = Map.get(claims, "exp")

    table = ensure_replay_table()

    purge_expired_nonces(table, now)

    if :ets.insert_new(table, {nonce, exp}) do
      :ok
    else
      {:error, :token_replayed}
    end
  end

  defp ensure_replay_table do
    case :ets.whereis(@replay_table) do
      :undefined ->
        :ets.new(@replay_table, [:named_table, :public, :set, read_concurrency: true])

      table ->
        table
    end
  rescue
    ArgumentError ->
      @replay_table
  end

  defp purge_expired_nonces(table, now) do
    ms = [{{:"$1", :"$2"}, [{:<, :"$2", now}], [true]}]
    :ets.select_delete(table, ms)
    :ok
  end

  defp claim_int(claims, key) do
    case Map.get(claims, key) do
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

  defp b64_decode(segment) do
    Base.url_decode64(segment, padding: false)
  end

  defp secret(opts) do
    case Keyword.get(opts, :secret) do
      value when is_binary(value) and byte_size(value) > 0 ->
        value

      _ ->
        Application.get_env(:openagents_runtime, :runtime_signature_secret) ||
          "dev-runtime-signature-secret"
    end
  end
end
