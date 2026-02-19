defmodule OpenAgentsRuntime.Tools.Comms.Providers.ResendAdapter do
  @moduledoc """
  Resend provider adapter for comms tool-pack runtime execution.
  """

  @behaviour OpenAgentsRuntime.Tools.Comms.ProviderAdapter

  alias OpenAgentsRuntime.Security.Sanitizer

  @resend_endpoint "https://api.resend.com/emails"
  @default_timeout_ms 10_000

  @impl true
  def send(request, _manifest, opts) when is_map(request) and is_list(opts) do
    request = normalize_map(request)
    endpoint = Keyword.get(opts, :endpoint, @resend_endpoint)
    timeout_ms = Keyword.get(opts, :timeout_ms, @default_timeout_ms)
    http_client = Keyword.get(opts, :http_client, &default_http_post/4)

    with {:ok, api_key} <- fetch_api_key(opts),
         {:ok, payload} <- build_payload(request, opts),
         {:ok, status, response_body} <- http_client.(endpoint, api_key, payload, timeout_ms) do
      map_response(status, response_body)
    else
      {:error, :missing_api_key} ->
        {:error, error_result("policy_denied.explicit_deny", nil, "missing_api_key")}

      {:error, {:invalid_payload, message}} ->
        {:error, error_result("manifest_validation.invalid_schema", nil, message)}

      {:error, {:transport, reason}} ->
        {:error, error_result("comms_failed.provider_error", nil, inspect(reason))}
    end
  end

  def send(_request, _manifest, _opts) do
    {:error, error_result("manifest_validation.invalid_schema", nil, "request must be an object")}
  end

  defp fetch_api_key(opts) do
    case Keyword.get(opts, :api_key) do
      value when is_binary(value) and value != "" -> {:ok, value}
      _ -> {:error, :missing_api_key}
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
          |> maybe_put("tags", [
            %{"name" => "template_id", "value" => template_id},
            %{"name" => "integration_id", "value" => request["integration_id"] || "unknown"}
          ])

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

  defp default_http_post(endpoint, api_key, payload, timeout_ms) do
    :inets.start()
    :ssl.start()

    headers = [
      {~c"authorization", to_charlist("Bearer #{api_key}")},
      {~c"content-type", ~c"application/json"}
    ]

    body = Jason.encode!(payload) |> to_charlist()
    request = {to_charlist(endpoint), headers, ~c"application/json", body}
    http_opts = [timeout: timeout_ms, connect_timeout: timeout_ms]

    case :httpc.request(:post, request, http_opts, body_format: :binary) do
      {:ok, {{_http_version, status, _reason_phrase}, _response_headers, response_body}} ->
        {:ok, status, response_body}

      {:error, reason} ->
        {:error, {:transport, reason}}
    end
  end

  defp normalize_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)
end
