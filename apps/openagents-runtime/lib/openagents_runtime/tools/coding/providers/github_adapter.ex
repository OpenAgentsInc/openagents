defmodule OpenAgentsRuntime.Tools.Coding.Providers.GitHubAdapter do
  @moduledoc """
  GitHub provider adapter for coding tool-pack runtime operations.
  """

  @behaviour OpenAgentsRuntime.Tools.Coding.ProviderAdapter

  alias OpenAgentsRuntime.Integrations.LaravelSecretClient
  alias OpenAgentsRuntime.Security.Sanitizer
  alias OpenAgentsRuntime.Tools.Network.GuardedHTTP

  @github_api_base "https://api.github.com"
  @default_timeout_ms 10_000
  @default_max_redirects 2
  @default_allowed_hosts ["api.github.com"]

  @impl true
  def execute(request, _manifest, opts) when is_map(request) and is_list(opts) do
    request = normalize_map(request)
    endpoint_base = Keyword.get(opts, :endpoint_base, @github_api_base)
    timeout_ms = Keyword.get(opts, :timeout_ms, @default_timeout_ms)

    with {:ok, api_token} <- fetch_api_token(request, opts),
         {:ok, operation_spec} <- build_operation_spec(request, endpoint_base),
         {:ok, status, response_body} <-
           invoke_request(operation_spec, api_token, timeout_ms, request, opts) do
      map_response(operation_spec, status, response_body)
    else
      {:error, :missing_api_token} ->
        {:error, error_result("policy_denied.explicit_deny", nil, "missing_api_token")}

      {:error, {:secret_fetch_failed, reason}} ->
        {:error,
         error_result("coding_failed.provider_error", nil, secret_fetch_error_message(reason))}

      {:error, {:invalid_request, message}} ->
        {:error, error_result("manifest_validation.invalid_schema", nil, message)}

      {:error, {:blocked, reason_code, details}} ->
        {:error,
         error_result(reason_code, nil, details["message"] || "outbound request blocked")
         |> Map.put("ssrf_block_reason", reason_code)}

      {:error, {:transport, reason}} ->
        {:error, error_result("coding_failed.provider_error", nil, inspect(reason))}
    end
  end

  def execute(_request, _manifest, _opts) do
    {:error, error_result("manifest_validation.invalid_schema", nil, "request must be an object")}
  end

  defp fetch_api_token(request, opts) do
    case Keyword.get(opts, :api_token) do
      value when is_binary(value) and value != "" -> {:ok, value}
      _ -> fetch_api_token_from_runtime_scope(request, opts)
    end
  end

  defp fetch_api_token_from_runtime_scope(request, opts) do
    with {:ok, scope} <- build_secret_scope(request, opts),
         secret_client <- Keyword.get(opts, :secret_client, LaravelSecretClient),
         secret_opts <- Keyword.get(opts, :secret_client_opts, []),
         {:ok, api_token} <- secret_client.fetch_secret("github", scope, secret_opts),
         true <- is_binary(api_token) and String.trim(api_token) != "" do
      {:ok, api_token}
    else
      {:error, :secret_not_found} -> {:error, :missing_api_token}
      {:error, :unauthorized} -> {:error, :missing_api_token}
      {:error, :invalid_scope} -> {:error, :missing_api_token}
      {:error, :misconfigured} -> {:error, :missing_api_token}
      {:error, reason} -> {:error, {:secret_fetch_failed, reason}}
      false -> {:error, :missing_api_token}
      _ -> {:error, :missing_api_token}
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

  defp build_operation_spec(request, endpoint_base) do
    operation = normalize_optional_string(request["operation"])
    repository = normalize_optional_string(request["repository"])

    cond do
      not valid_repository?(repository) ->
        {:error,
         {:invalid_request,
          "repository is required and must be in 'owner/repo' format for github operations"}}

      operation == "get_issue" ->
        with {:ok, issue_number} <- positive_integer(request["issue_number"]) do
          {:ok,
           %{
             "operation" => operation,
             "method" => :get,
             "url" => endpoint_base <> "/repos/" <> repository <> "/issues/" <> issue_number,
             "body" => ""
           }}
        else
          :error -> {:error, {:invalid_request, "issue_number must be a positive integer"}}
        end

      operation == "get_pull_request" ->
        with {:ok, pull_number} <- positive_integer(request["pull_number"]) do
          {:ok,
           %{
             "operation" => operation,
             "method" => :get,
             "url" => endpoint_base <> "/repos/" <> repository <> "/pulls/" <> pull_number,
             "body" => ""
           }}
        else
          :error -> {:error, {:invalid_request, "pull_number must be a positive integer"}}
        end

      operation == "add_issue_comment" ->
        with {:ok, issue_number} <- positive_integer(request["issue_number"]),
             {:ok, comment_body} <- non_empty_string(request["body"], "body is required") do
          {:ok,
           %{
             "operation" => operation,
             "method" => :post,
             "url" =>
               endpoint_base <>
                 "/repos/" <>
                 repository <>
                 "/issues/" <>
                 issue_number <>
                 "/comments",
             "body" => Jason.encode!(%{"body" => comment_body})
           }}
        else
          :error -> {:error, {:invalid_request, "issue_number must be a positive integer"}}
          {:error, message} -> {:error, {:invalid_request, message}}
        end

      true ->
        {:error,
         {:invalid_request,
          "unsupported github coding operation: #{inspect(operation || request["operation"])}"}}
    end
  end

  defp invoke_request(operation_spec, api_token, timeout_ms, request, opts) do
    headers =
      [
        {"authorization", "Bearer #{api_token}"},
        {"accept", "application/vnd.github+json"},
        {"x-github-api-version", "2022-11-28"},
        {"user-agent", "runtime"}
      ]
      |> maybe_put_content_type(operation_spec["method"])

    guard_opts = network_guard_opts(request, operation_spec["url"], opts)

    GuardedHTTP.request(
      operation_spec["method"],
      operation_spec["url"],
      headers,
      operation_spec["body"],
      Keyword.merge(guard_opts, timeout_ms: timeout_ms, connect_timeout_ms: timeout_ms)
    )
  end

  defp map_response(operation_spec, status, response_body) when status >= 200 and status < 300 do
    operation = operation_spec["operation"]
    payload = response_body |> decode_json() |> Sanitizer.sanitize()

    result =
      case operation do
        "add_issue_comment" ->
          %{
            "comment_id" => payload["id"],
            "comment_url" => payload["html_url"],
            "state" => "succeeded"
          }

        _ ->
          %{
            "state" => "succeeded",
            "data" => payload
          }
      end

    {:ok,
     result
     |> Map.put("reason_code", "policy_allowed.default")
     |> Map.put("provider_status", status)
     |> Map.put("operation", operation)}
  end

  defp map_response(_operation_spec, status, response_body) do
    body = response_body |> decode_json() |> Sanitizer.sanitize()

    reason_code =
      case status do
        400 -> "manifest_validation.invalid_schema"
        401 -> "policy_denied.explicit_deny"
        403 -> "policy_denied.explicit_deny"
        404 -> "coding_failed.provider_error"
        422 -> "manifest_validation.invalid_schema"
        429 -> "policy_denied.budget_exhausted"
        _ -> "coding_failed.provider_error"
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

  defp network_guard_opts(request, url, opts) do
    endpoint_host = endpoint_host(url)

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
        provider: "github"
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

  defp maybe_put_content_type(headers, :post),
    do: headers ++ [{"content-type", "application/json"}]

  defp maybe_put_content_type(headers, :patch),
    do: headers ++ [{"content-type", "application/json"}]

  defp maybe_put_content_type(headers, _method), do: headers

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

  defp normalize_optional_string(value) when is_integer(value), do: Integer.to_string(value)

  defp normalize_optional_string(_), do: nil

  defp non_empty_string(value, error_message) when is_binary(value) do
    trimmed = String.trim(value)
    if trimmed == "", do: {:error, error_message}, else: {:ok, trimmed}
  end

  defp non_empty_string(_value, error_message), do: {:error, error_message}

  defp positive_integer(value) when is_integer(value) and value > 0,
    do: {:ok, Integer.to_string(value)}

  defp positive_integer(value) when is_binary(value) do
    case Integer.parse(String.trim(value)) do
      {parsed, ""} when parsed > 0 -> {:ok, Integer.to_string(parsed)}
      _ -> :error
    end
  end

  defp positive_integer(_value), do: :error

  defp valid_repository?(repository) when is_binary(repository) do
    String.match?(repository, ~r/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/)
  end

  defp valid_repository?(_), do: false

  defp secret_fetch_error_message(reason) do
    reason
    |> inspect()
    |> String.replace_prefix(":", "")
    |> then(&("runtime_secret_fetch_failed:" <> &1))
  end
end
