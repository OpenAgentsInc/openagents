defmodule OpenAgentsRuntime.Tools.Comms.Providers.ResendAdapter do
  @moduledoc """
  Resend provider adapter for comms tool-pack runtime execution.
  """

  @behaviour OpenAgentsRuntime.Tools.Comms.ProviderAdapter

  alias OpenAgentsRuntime.Integrations.LaravelSecretClient
  alias OpenAgentsRuntime.Security.Sanitizer
  alias OpenAgentsRuntime.Tools.Network.GuardedHTTP

  @resend_endpoint "https://api.resend.com/emails"
  @default_timeout_ms 10_000
  @default_max_redirects 2
  @default_allowed_hosts ["api.resend.com"]

  @impl true
  def send(request, _manifest, opts) when is_map(request) and is_list(opts) do
    request = normalize_map(request)
    endpoint = Keyword.get(opts, :endpoint, @resend_endpoint)
    timeout_ms = Keyword.get(opts, :timeout_ms, @default_timeout_ms)
    http_client = Keyword.get(opts, :http_client, &guarded_http_post/5)
    guard_opts = network_guard_opts(request, endpoint, opts)

    with {:ok, api_key} <- fetch_api_key(request, opts),
         {:ok, payload} <- build_payload(request, opts),
         {:ok, status, response_body} <-
           invoke_http_client(http_client, endpoint, api_key, payload, timeout_ms, guard_opts) do
      map_response(status, response_body)
    else
      {:error, :missing_api_key} ->
        {:error, error_result("policy_denied.explicit_deny", nil, "missing_api_key")}

      {:error, {:secret_fetch_failed, reason}} ->
        {:error,
         error_result("comms_failed.provider_error", nil, secret_fetch_error_message(reason))}

      {:error, {:invalid_payload, message}} ->
        {:error, error_result("manifest_validation.invalid_schema", nil, message)}

      {:error, {:blocked, reason_code, details}} ->
        {:error,
         error_result(reason_code, nil, details["message"] || "outbound request blocked")
         |> maybe_put("ssrf_block_reason", reason_code)}

      {:error, {:transport, reason}} ->
        {:error, error_result("comms_failed.provider_error", nil, inspect(reason))}
    end
  end

  def send(_request, _manifest, _opts) do
    {:error, error_result("manifest_validation.invalid_schema", nil, "request must be an object")}
  end

  defp fetch_api_key(request, opts) do
    case Keyword.get(opts, :api_key) do
      value when is_binary(value) and value != "" -> {:ok, value}
      _ -> fetch_api_key_from_runtime_scope(request, opts)
    end
  end

  defp fetch_api_key_from_runtime_scope(request, opts) do
    with {:ok, scope} <- build_secret_scope(request, opts),
         secret_client <- Keyword.get(opts, :secret_client, LaravelSecretClient),
         secret_opts <- Keyword.get(opts, :secret_client_opts, []),
         {:ok, api_key} <- secret_client.fetch_secret("resend", scope, secret_opts),
         true <- is_binary(api_key) and String.trim(api_key) != "" do
      {:ok, api_key}
    else
      {:error, :secret_not_found} -> {:error, :missing_api_key}
      {:error, :unauthorized} -> {:error, :missing_api_key}
      {:error, :invalid_scope} -> {:error, :missing_api_key}
      {:error, :misconfigured} -> {:error, :missing_api_key}
      {:error, reason} -> {:error, {:secret_fetch_failed, reason}}
      false -> {:error, :missing_api_key}
      _ -> {:error, :missing_api_key}
    end
  end

  defp build_secret_scope(request, opts) do
    user_id = request["user_id"] || Keyword.get(opts, :user_id)
    integration_id = request["integration_id"] || Keyword.get(opts, :integration_id)
    run_id = request["run_id"] || Keyword.get(opts, :run_id)
    tool_call_id = request["tool_call_id"] || Keyword.get(opts, :tool_call_id)
    org_id = request["org_id"] || Keyword.get(opts, :org_id)

    with {:ok, user_id} <- normalize_user_id(user_id),
         true <- present_string?(integration_id),
         true <- present_string?(run_id),
         true <- present_string?(tool_call_id) do
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

  defp build_payload(request, opts) do
    from = request["from"] || Keyword.get(opts, :from)
    recipient = request["recipient"]
    template_id = request["template_id"]
    variables = request["variables"] || %{}
    html = request["html"]
    text = request["text"] || "template=#{template_id} vars=#{Jason.encode!(variables)}"
    subject = request["subject"] || "OpenAgents message (#{template_id})"

    cond do
      not is_binary(from) or String.trim(from) == "" ->
        {:error, {:invalid_payload, "from is required"}}

      not is_binary(recipient) or String.trim(recipient) == "" ->
        {:error, {:invalid_payload, "recipient is required"}}

      not is_binary(template_id) or String.trim(template_id) == "" ->
        {:error, {:invalid_payload, "template_id is required"}}

      not is_map(variables) ->
        {:error, {:invalid_payload, "variables must be an object"}}

      true ->
        payload =
          %{
            "from" => from,
            "to" => [recipient],
            "subject" => subject,
            "text" => text
          }
          |> maybe_put("html", html)
          |> maybe_put("tags", build_tags(request, template_id))

        {:ok, payload}
    end
  end

  defp map_response(status, response_body) when status >= 200 and status < 300 do
    body = response_body |> decode_json() |> Sanitizer.sanitize()
    message_id = body["id"] || "resend_unknown"

    {:ok,
     %{
       "message_id" => message_id,
       "state" => "sent",
       "reason_code" => "policy_allowed.default",
       "provider_status" => status,
       "provider_body" => body
     }}
  end

  defp map_response(status, response_body) do
    body = response_body |> decode_json() |> Sanitizer.sanitize()

    reason_code =
      case status do
        400 -> "manifest_validation.invalid_schema"
        401 -> "policy_denied.explicit_deny"
        403 -> "policy_denied.explicit_deny"
        422 -> "manifest_validation.invalid_schema"
        429 -> "policy_denied.budget_exhausted"
        _ -> "comms_failed.provider_error"
      end

    {:error,
     error_result(reason_code, status, body |> extract_error_message() |> Sanitizer.sanitize())}
  end

  defp error_result(reason_code, provider_status, message) do
    %{
      "reason_code" => reason_code,
      "state" => "failed",
      "provider_status" => provider_status,
      "message" => message
    }
  end

  defp extract_error_message(body) when is_map(body) do
    body["message"] || body["error"] || Jason.encode!(body)
  end

  defp extract_error_message(body) when is_binary(body), do: body
  defp extract_error_message(_), do: "provider_error"

  defp decode_json(body) when is_binary(body) do
    case Jason.decode(body) do
      {:ok, decoded} when is_map(decoded) -> decoded
      _ -> %{"raw" => body}
    end
  end

  defp decode_json(_), do: %{}

  defp guarded_http_post(endpoint, api_key, payload, timeout_ms, guard_opts) do
    headers = [
      {"authorization", "Bearer #{api_key}"},
      {"content-type", "application/json"}
    ]

    GuardedHTTP.post_json(
      endpoint,
      headers,
      payload,
      Keyword.merge(guard_opts, timeout_ms: timeout_ms, connect_timeout_ms: timeout_ms)
    )
  end

  defp invoke_http_client(http_client, endpoint, api_key, payload, timeout_ms, guard_opts) do
    cond do
      is_function(http_client, 5) ->
        http_client.(endpoint, api_key, payload, timeout_ms, guard_opts)

      is_function(http_client, 4) ->
        http_client.(endpoint, api_key, payload, timeout_ms)

      true ->
        {:error, {:transport, :invalid_http_client}}
    end
  end

  defp network_guard_opts(request, endpoint, opts) do
    endpoint_host = endpoint_host(endpoint)

    allowed_hosts =
      opts
      |> Keyword.get(:allowed_hosts, @default_allowed_hosts)
      |> normalize_allowlist(endpoint_host)

    [
      allowed_hosts: allowed_hosts,
      max_redirects: Keyword.get(opts, :max_redirects, @default_max_redirects),
      guard_enabled: Keyword.get(opts, :guard_enabled),
      dns_resolver: Keyword.get(opts, :dns_resolver),
      transport: Keyword.get(opts, :network_transport),
      audit_metadata: %{
        run_id: request["run_id"],
        tool_call_id: request["tool_call_id"],
        integration_id: request["integration_id"],
        provider: "resend"
      }
    ]
    |> Enum.reject(fn
      {:guard_enabled, nil} -> true
      {:dns_resolver, nil} -> true
      {:transport, nil} -> true
      _ -> false
    end)
  end

  defp endpoint_host(endpoint) when is_binary(endpoint) do
    endpoint
    |> URI.parse()
    |> Map.get(:host)
    |> normalize_optional_string()
  rescue
    _ -> nil
  end

  defp normalize_allowlist(allowlist, endpoint_host) when is_list(allowlist) do
    hosts =
      allowlist
      |> Enum.flat_map(fn
        value when is_binary(value) ->
          trimmed = String.trim(value)
          if trimmed == "", do: [], else: [trimmed]

        _ ->
          []
      end)

    if hosts == [] and is_binary(endpoint_host) and endpoint_host != "" do
      [endpoint_host]
    else
      hosts
    end
  end

  defp normalize_allowlist(_allowlist, endpoint_host) when is_binary(endpoint_host) do
    [endpoint_host]
  end

  defp normalize_allowlist(_allowlist, _endpoint_host), do: @default_allowed_hosts

  defp normalize_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp normalize_user_id(value) when is_integer(value) and value > 0, do: {:ok, value}

  defp normalize_user_id(value) when is_binary(value) do
    case Integer.parse(value) do
      {parsed, ""} when parsed > 0 -> {:ok, parsed}
      _ -> :error
    end
  end

  defp normalize_user_id(_), do: :error

  defp present_string?(nil), do: false
  defp present_string?(value) when is_binary(value), do: String.trim(value) != ""

  defp present_string?(value) when is_atom(value),
    do: value |> Atom.to_string() |> present_string?()

  defp present_string?(_), do: false

  defp normalize_optional_string(nil), do: nil

  defp normalize_optional_string(value) when is_binary(value) do
    trimmed = String.trim(value)
    if trimmed == "", do: nil, else: trimmed
  end

  defp normalize_optional_string(value) when is_atom(value),
    do: value |> Atom.to_string() |> normalize_optional_string()

  defp normalize_optional_string(value), do: value |> to_string() |> normalize_optional_string()

  defp secret_fetch_error_message(reason) when is_atom(reason),
    do: "runtime_secret_fetch_failed:#{reason}"

  defp secret_fetch_error_message(_reason), do: "runtime_secret_fetch_failed"

  defp build_tags(request, template_id) do
    [%{"name" => "template_id", "value" => template_id}]
    |> append_tag("integration_id", request["integration_id"] || "unknown")
    |> append_tag("user_id", request["user_id"])
    |> append_tag("run_id", request["run_id"])
    |> append_tag("tool_call_id", request["tool_call_id"])
  end

  defp append_tag(tags, _name, nil), do: tags

  defp append_tag(tags, name, value) do
    tag_value =
      cond do
        is_binary(value) -> value
        is_integer(value) -> Integer.to_string(value)
        is_float(value) -> :erlang.float_to_binary(value, [:compact, decimals: 6])
        is_atom(value) -> Atom.to_string(value)
        true -> to_string(value)
      end

    if String.trim(tag_value) == "" do
      tags
    else
      tags ++ [%{"name" => name, "value" => tag_value}]
    end
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)
end
