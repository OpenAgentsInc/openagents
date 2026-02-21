defmodule OpenAgentsRuntime.Integrations.LaravelSecretClient do
  @moduledoc """
  Fetches provider secrets from Laravel via an internal signed endpoint.

  Secrets are cached only in short-lived ETS memory and keyed to execution scope.
  """

  @cache_table :openagents_runtime_laravel_secret_cache
  @default_timeout_ms 2_500
  @default_cache_ttl_ms 60_000

  @type fetch_error ::
          :invalid_scope
          | :misconfigured
          | :secret_not_found
          | :unauthorized
          | :transport_error
          | :upstream_error
          | :invalid_response

  @type request_fn ::
          (url :: String.t(),
           headers :: map(),
           body :: String.t(),
           timeout_ms :: non_neg_integer() ->
             {:ok, pos_integer(), String.t()} | {:error, term()})

  @type fetch_opt ::
          {:request_fn, request_fn()}
          | {:base_url, String.t()}
          | {:secret_fetch_path, String.t()}
          | {:shared_secret, String.t()}
          | {:key_id, String.t()}
          | {:signature_ttl_seconds, pos_integer()}
          | {:request_timeout_ms, pos_integer()}
          | {:default_secret_cache_ttl_ms, non_neg_integer()}
          | {:now_ms, non_neg_integer()}

  @spec fetch_secret(String.t(), map(), [fetch_opt()]) ::
          {:ok, String.t()} | {:error, fetch_error()}
  def fetch_secret(provider, scope, opts \\ []) when is_binary(provider) and is_map(scope) do
    with {:ok, normalized_scope} <- normalize_scope(scope),
         cache_key <- cache_key(provider, normalized_scope),
         {:cache_miss, now_ms} <- fetch_from_cache(cache_key, opts),
         {:ok, secret, cache_ttl_ms} <- fetch_remote(provider, normalized_scope, opts, now_ms) do
      maybe_put_cache(cache_key, secret, cache_ttl_ms, now_ms)
      {:ok, secret}
    else
      {:ok, secret} -> {:ok, secret}
      {:error, reason} -> {:error, reason}
    end
  end

  @doc false
  @spec clear_cache() :: :ok
  def clear_cache do
    case :ets.whereis(@cache_table) do
      :undefined ->
        :ok

      table ->
        :ets.delete_all_objects(table)
        :ok
    end
  end

  defp fetch_remote(provider, scope, opts, now_ms) do
    with {:ok, config} <- runtime_config(opts),
         {:ok, payload} <- build_payload(provider, scope),
         {:ok, body, headers, url, timeout_ms} <- build_signed_request(payload, config, now_ms),
         {:ok, status, response_body} <- request_fn(opts).(url, headers, body, timeout_ms),
         {:ok, secret, cache_ttl_ms} <- parse_response(status, response_body, config) do
      {:ok, secret, cache_ttl_ms}
    else
      {:error, :secret_not_found} -> {:error, :secret_not_found}
      {:error, :unauthorized} -> {:error, :unauthorized}
      {:error, :invalid_response} -> {:error, :invalid_response}
      {:error, :misconfigured} -> {:error, :misconfigured}
      {:error, :invalid_scope} -> {:error, :invalid_scope}
      {:error, {:transport, _reason}} -> {:error, :transport_error}
      {:error, _reason} -> {:error, :upstream_error}
    end
  end

  defp normalize_scope(scope) when is_map(scope) do
    user_id = map_get(scope, "user_id")
    integration_id = map_get(scope, "integration_id")
    run_id = map_get(scope, "run_id")
    tool_call_id = map_get(scope, "tool_call_id")
    org_id = map_get(scope, "org_id")

    with {:ok, user_id} <- normalize_user_id(user_id),
         :ok <- ensure_non_empty(integration_id),
         :ok <- ensure_non_empty(run_id),
         :ok <- ensure_non_empty(tool_call_id) do
      {:ok,
       %{
         "user_id" => user_id,
         "integration_id" => to_string(integration_id),
         "run_id" => to_string(run_id),
         "tool_call_id" => to_string(tool_call_id),
         "org_id" => normalize_optional_string(org_id)
       }}
    else
      _ -> {:error, :invalid_scope}
    end
  end

  defp build_payload(provider, scope) do
    payload =
      %{
        "user_id" => scope["user_id"],
        "provider" => provider,
        "integration_id" => scope["integration_id"],
        "run_id" => scope["run_id"],
        "tool_call_id" => scope["tool_call_id"]
      }
      |> maybe_put("org_id", scope["org_id"])

    {:ok, payload}
  end

  defp build_signed_request(payload, config, now_ms) do
    body = Jason.encode!(payload)
    body_hash = sha256_hex(body)
    timestamp = Integer.to_string(div(now_ms, 1_000))
    nonce = random_nonce()

    signature =
      :crypto.mac(:hmac, :sha256, config.shared_secret, "#{timestamp}\n#{nonce}\n#{body_hash}")
      |> Base.encode16(case: :lower)

    headers = %{
      "content-type" => "application/json",
      "accept" => "application/json",
      "x-oa-internal-key-id" => config.key_id,
      "x-oa-internal-timestamp" => timestamp,
      "x-oa-internal-nonce" => nonce,
      "x-oa-internal-body-sha256" => body_hash,
      "x-oa-internal-signature" => signature,
      "x-oa-internal-signature-ttl" => Integer.to_string(config.signature_ttl_seconds)
    }

    url = join_url(config.base_url, config.secret_fetch_path)

    {:ok, body, headers, url, config.request_timeout_ms}
  end

  defp parse_response(status, response_body, config) when status >= 200 and status < 300 do
    with {:ok, decoded} <- Jason.decode(response_body),
         %{"data" => data} when is_map(data) <- decoded,
         secret when is_binary(secret) and secret != "" <- data["secret"] do
      cache_ttl_ms =
        case data["cache_ttl_ms"] do
          value when is_integer(value) and value >= 0 -> value
          _ -> config.default_secret_cache_ttl_ms
        end

      {:ok, secret, cache_ttl_ms}
    else
      _ -> {:error, :invalid_response}
    end
  end

  defp parse_response(404, _response_body, _config), do: {:error, :secret_not_found}

  defp parse_response(status, _response_body, _config) when status in [401, 403],
    do: {:error, :unauthorized}

  defp parse_response(_status, _response_body, _config), do: {:error, :upstream_error}

  defp request_fn(opts), do: Keyword.get(opts, :request_fn, &default_request/4)

  defp fetch_from_cache(cache_key, opts) do
    now_ms = Keyword.get(opts, :now_ms, System.monotonic_time(:millisecond))
    table = ensure_cache_table()

    case :ets.lookup(table, cache_key) do
      [{^cache_key, secret, expires_at_ms}] when is_binary(secret) and expires_at_ms > now_ms ->
        {:ok, secret}

      [{^cache_key, _secret, _expires_at_ms}] ->
        :ets.delete(table, cache_key)
        {:cache_miss, now_ms}

      _ ->
        {:cache_miss, now_ms}
    end
  end

  defp maybe_put_cache(_cache_key, _secret, cache_ttl_ms, _now_ms) when cache_ttl_ms <= 0, do: :ok

  defp maybe_put_cache(cache_key, secret, cache_ttl_ms, now_ms) do
    expires_at_ms = now_ms + cache_ttl_ms
    table = ensure_cache_table()
    true = :ets.insert(table, {cache_key, secret, expires_at_ms})
    :ok
  end

  defp cache_key(provider, scope) do
    {
      provider,
      scope["user_id"],
      scope["integration_id"],
      scope["run_id"],
      scope["tool_call_id"],
      scope["org_id"]
    }
  end

  defp ensure_cache_table do
    case :ets.whereis(@cache_table) do
      :undefined ->
        :ets.new(@cache_table, [:named_table, :public, :set, read_concurrency: true])

      table ->
        table
    end
  rescue
    ArgumentError ->
      @cache_table
  end

  defp runtime_config(opts) do
    base_url = config_value(opts, :base_url, "http://openagents.com")

    secret_fetch_path =
      config_value(opts, :secret_fetch_path, "/api/internal/runtime/integrations/secrets/fetch")

    shared_secret = config_value(opts, :shared_secret, "")
    key_id = config_value(opts, :key_id, "runtime-internal-v1")
    signature_ttl_seconds = config_value(opts, :signature_ttl_seconds, 60)
    request_timeout_ms = config_value(opts, :request_timeout_ms, @default_timeout_ms)

    default_secret_cache_ttl_ms =
      config_value(opts, :default_secret_cache_ttl_ms, @default_cache_ttl_ms)

    cond do
      not is_binary(base_url) or String.trim(base_url) == "" ->
        {:error, :misconfigured}

      not is_binary(secret_fetch_path) or String.trim(secret_fetch_path) == "" ->
        {:error, :misconfigured}

      not is_binary(shared_secret) or String.trim(shared_secret) == "" ->
        {:error, :misconfigured}

      not is_binary(key_id) or String.trim(key_id) == "" ->
        {:error, :misconfigured}

      not is_integer(signature_ttl_seconds) or signature_ttl_seconds <= 0 ->
        {:error, :misconfigured}

      not is_integer(request_timeout_ms) or request_timeout_ms <= 0 ->
        {:error, :misconfigured}

      not is_integer(default_secret_cache_ttl_ms) or default_secret_cache_ttl_ms < 0 ->
        {:error, :misconfigured}

      true ->
        {:ok,
         %{
           base_url: base_url,
           secret_fetch_path: secret_fetch_path,
           shared_secret: shared_secret,
           key_id: key_id,
           signature_ttl_seconds: signature_ttl_seconds,
           request_timeout_ms: request_timeout_ms,
           default_secret_cache_ttl_ms: default_secret_cache_ttl_ms
         }}
    end
  end

  defp config_value(opts, key, default) do
    case Keyword.get(opts, key) do
      nil ->
        case Application.get_env(:openagents_runtime, :laravel_internal, %{}) do
          %{} = map -> Map.get(map, key, default)
          list when is_list(list) -> Keyword.get(list, key, default)
          _ -> default
        end

      value ->
        value
    end
  end

  defp default_request(url, headers, body, timeout_ms) do
    :inets.start()
    :ssl.start()

    request =
      {to_charlist(url), normalize_http_headers(headers), ~c"application/json", to_charlist(body)}

    http_opts = [timeout: timeout_ms, connect_timeout: timeout_ms]

    case :httpc.request(:post, request, http_opts, body_format: :binary) do
      {:ok, {{_http_version, status, _reason_phrase}, _response_headers, response_body}} ->
        {:ok, status, IO.iodata_to_binary(response_body)}

      {:error, reason} ->
        {:error, {:transport, reason}}
    end
  end

  defp normalize_http_headers(headers) when is_map(headers) do
    Enum.map(headers, fn {name, value} ->
      {name |> to_string() |> String.downcase() |> to_charlist(),
       to_string(value) |> to_charlist()}
    end)
  end

  defp normalize_user_id(value) when is_integer(value) and value > 0, do: {:ok, value}

  defp normalize_user_id(value) when is_binary(value) do
    case Integer.parse(value) do
      {parsed, ""} when parsed > 0 -> {:ok, parsed}
      _ -> :error
    end
  end

  defp normalize_user_id(_value), do: :error

  defp ensure_non_empty(nil), do: :error

  defp ensure_non_empty(value) when is_binary(value),
    do: if(String.trim(value) == "", do: :error, else: :ok)

  defp ensure_non_empty(value) when is_atom(value),
    do: value |> Atom.to_string() |> ensure_non_empty()

  defp ensure_non_empty(_value), do: :error

  defp normalize_optional_string(nil), do: nil

  defp normalize_optional_string(value) when is_binary(value) do
    trimmed = String.trim(value)
    if trimmed == "", do: nil, else: trimmed
  end

  defp normalize_optional_string(value) when is_atom(value),
    do: value |> Atom.to_string() |> normalize_optional_string()

  defp normalize_optional_string(value), do: value |> to_string() |> normalize_optional_string()

  defp map_get(map, key) do
    case key do
      "user_id" -> Map.get(map, "user_id") || Map.get(map, :user_id)
      "integration_id" -> Map.get(map, "integration_id") || Map.get(map, :integration_id)
      "run_id" -> Map.get(map, "run_id") || Map.get(map, :run_id)
      "tool_call_id" -> Map.get(map, "tool_call_id") || Map.get(map, :tool_call_id)
      "org_id" -> Map.get(map, "org_id") || Map.get(map, :org_id)
      _ -> Map.get(map, key)
    end
  end

  defp join_url(base_url, path) do
    String.trim_trailing(base_url, "/") <> "/" <> String.trim_leading(path, "/")
  end

  defp random_nonce do
    "nonce_" <> Base.encode16(:crypto.strong_rand_bytes(12), case: :lower)
  end

  defp sha256_hex(value) when is_binary(value) do
    :crypto.hash(:sha256, value)
    |> Base.encode16(case: :lower)
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)
end
