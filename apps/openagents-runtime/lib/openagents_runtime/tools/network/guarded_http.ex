defmodule OpenAgentsRuntime.Tools.Network.GuardedHTTP do
  @moduledoc """
  Guarded outbound HTTP seam for runtime tool integrations.

  Enforces:
  - host allowlists/pattern checks
  - metadata endpoint blocking
  - private/internal address blocking
  - DNS pinning across redirects
  - bounded redirect following
  """

  alias OpenAgentsRuntime.Security.Sanitizer
  alias OpenAgentsRuntime.Telemetry.Events

  @default_timeout_ms 10_000
  @default_connect_timeout_ms 10_000
  @default_max_redirects 2

  @metadata_hosts MapSet.new([
                    "metadata",
                    "metadata.google.internal",
                    "metadata.google.internal.",
                    "metadata.aliyun.internal",
                    "169.254.169.254"
                  ])

  @redirect_statuses MapSet.new([301, 302, 303, 307, 308])

  @type blocked_reason :: String.t()

  @type result ::
          {:ok, non_neg_integer(), binary()}
          | {:error, {:blocked, blocked_reason(), map()}}
          | {:error, {:transport, term()}}

  @type request_opt ::
          {:timeout_ms, pos_integer()}
          | {:connect_timeout_ms, pos_integer()}
          | {:max_redirects, non_neg_integer()}
          | {:allowed_hosts, [String.t()]}
          | {:guard_enabled, boolean()}
          | {:audit_metadata, map()}
          | {:dns_resolver, (String.t() -> {:ok, [ip_address()]} | {:error, term()})}
          | {:transport,
             (atom(), String.t(), [{String.t(), String.t()}], binary(), keyword() ->
                {:ok, non_neg_integer(), [{String.t(), String.t()}], binary()}
                | {:error, term()})}

  @type ip_address :: :inet.ip_address()

  @spec post_json(String.t(), [{String.t(), String.t()}], map(), [request_opt()]) :: result()
  def post_json(url, headers, payload, opts \\ [])
      when is_binary(url) and is_list(headers) and is_map(payload) do
    body = Jason.encode!(payload)
    request(:post, url, headers, body, opts)
  end

  @spec request(atom(), String.t(), [{String.t(), String.t()}], binary(), [request_opt()]) ::
          result()
  def request(method, url, headers, body, opts \\ [])
      when is_atom(method) and is_binary(url) and is_list(headers) and is_binary(body) do
    normalized_headers = normalize_headers(headers)
    guard_enabled = guard_enabled?(opts)
    transport = Keyword.get(opts, :transport, &default_transport/5)

    if guard_enabled do
      do_request(
        method,
        url,
        normalized_headers,
        body,
        opts,
        transport,
        Keyword.get(opts, :max_redirects, @default_max_redirects),
        %{}
      )
    else
      transport_opts = transport_opts(opts, nil, [])

      case call_transport(method, url, normalized_headers, body, transport, transport_opts) do
        {:ok, status, _response_headers, response_body} -> {:ok, status, response_body}
        {:error, reason} -> {:error, reason}
      end
    end
  end

  defp do_request(method, url, headers, body, opts, transport, redirects_left, dns_pins) do
    with {:ok, uri} <- parse_uri(url, opts),
         {:ok, host} <- normalized_host(uri, url, opts),
         :ok <- enforce_not_metadata_host(url, host, opts),
         {:ok, resolved_ips} <- resolve_host(url, host, opts),
         :ok <- enforce_not_metadata_ip(url, host, resolved_ips, opts),
         :ok <- enforce_public_ips(url, host, resolved_ips, opts),
         {:ok, next_pins} <- enforce_dns_pin(url, host, resolved_ips, dns_pins, opts),
         :ok <- enforce_host_allowlist(url, host, opts),
         {:ok, status, response_headers, response_body} <-
           call_transport(
             method,
             url,
             headers,
             body,
             transport,
             transport_opts(opts, host, resolved_ips)
           ) do
      if MapSet.member?(@redirect_statuses, status) do
        follow_redirect(
          method,
          url,
          headers,
          body,
          opts,
          transport,
          redirects_left,
          next_pins,
          response_headers
        )
      else
        {:ok, status, response_body}
      end
    end
  end

  defp follow_redirect(
         _method,
         url,
         _headers,
         _body,
         opts,
         _transport,
         redirects_left,
         _dns_pins,
         _response_headers
       )
       when redirects_left <= 0 do
    block(
      "ssrf_block.redirect_limit_exceeded",
      url,
      nil,
      "redirect limit exceeded",
      opts
    )
  end

  defp follow_redirect(
         method,
         url,
         headers,
         body,
         opts,
         transport,
         redirects_left,
         dns_pins,
         response_headers
       ) do
    case location_header(response_headers) do
      nil ->
        block(
          "ssrf_block.redirect_missing_location",
          url,
          nil,
          "redirect response missing location header",
          opts
        )

      location ->
        with {:ok, next_url} <- resolve_redirect_url(url, location) do
          do_request(
            method,
            next_url,
            headers,
            body,
            opts,
            transport,
            redirects_left - 1,
            dns_pins
          )
        else
          {:error, :invalid_location} ->
            block(
              "ssrf_block.redirect_invalid_location",
              url,
              nil,
              "redirect location is invalid",
              opts
            )
        end
    end
  end

  defp enforce_host_allowlist(url, host, opts) do
    allowed_hosts =
      opts
      |> Keyword.get(:allowed_hosts, [])
      |> Enum.map(&normalize_host_value/1)
      |> Enum.reject(&is_nil/1)

    if allowed_hosts == [] or Enum.any?(allowed_hosts, &host_matches_pattern?(host, &1)) do
      :ok
    else
      block("ssrf_block.host_not_allowed", url, host, "host is not in allowlist", opts)
    end
  end

  defp enforce_not_metadata_host(url, host, opts) do
    if MapSet.member?(@metadata_hosts, host) do
      block("ssrf_block.metadata_endpoint", url, host, "metadata hostname blocked", opts)
    else
      :ok
    end
  end

  defp enforce_not_metadata_ip(url, host, ips, opts) do
    if Enum.any?(ips, &metadata_ip?/1) do
      block("ssrf_block.metadata_endpoint", url, host, "metadata IP blocked", opts)
    else
      :ok
    end
  end

  defp enforce_public_ips(url, host, ips, opts) do
    if Enum.any?(ips, &private_or_internal_ip?/1) do
      block("ssrf_block.private_address", url, host, "private/internal address blocked", opts)
    else
      :ok
    end
  end

  defp enforce_dns_pin(url, host, resolved_ips, dns_pins, opts) do
    current_pin =
      resolved_ips
      |> Enum.map(&ip_to_string/1)
      |> MapSet.new()

    case Map.get(dns_pins, host) do
      nil ->
        {:ok, Map.put(dns_pins, host, current_pin)}

      prior_pin ->
        if MapSet.disjoint?(prior_pin, current_pin) do
          block(
            "ssrf_block.dns_pin_mismatch",
            url,
            host,
            "DNS pin mismatch detected across requests",
            opts
          )
        else
          {:ok, dns_pins}
        end
    end
  end

  defp call_transport(method, url, headers, body, transport, transport_opts) do
    case transport.(method, url, headers, body, transport_opts) do
      {:ok, status, response_headers, response_body}
      when is_integer(status) and is_list(response_headers) and is_binary(response_body) ->
        {:ok, status, response_headers, response_body}

      {:ok, status, response_body} when is_integer(status) and is_binary(response_body) ->
        {:ok, status, [], response_body}

      {:error, {:blocked, _, _} = blocked} ->
        {:error, blocked}

      {:error, {:transport, _} = transport_error} ->
        {:error, transport_error}

      {:error, reason} ->
        {:error, {:transport, reason}}

      other ->
        {:error, {:transport, {:invalid_transport_response, other}}}
    end
    |> case do
      {:ok, status, response_headers, response_body} ->
        {:ok, status, response_headers, response_body}

      {:error, {:blocked, _code, _details} = blocked} ->
        {:error, blocked}

      {:error, {:transport, reason}} ->
        {:error, {:transport, reason}}
    end
  end

  defp default_transport(method, url, headers, body, opts) do
    :inets.start()
    :ssl.start()

    request = build_httpc_request(method, url, headers, body)
    http_opts = build_httpc_options(opts)
    request_opts = [body_format: :binary]

    case :httpc.request(method, request, http_opts, request_opts) do
      {:ok, {{_http_version, status, _reason_phrase}, response_headers, response_body}} ->
        {:ok, status, normalize_headers(response_headers), response_body}

      {:error, reason} ->
        {:error, {:transport, reason}}
    end
  end

  defp build_httpc_request(method, url, headers, body) do
    char_url = String.to_charlist(url)

    char_headers =
      Enum.map(headers, fn {key, value} ->
        {String.to_charlist(key), String.to_charlist(value)}
      end)

    case method do
      :post -> {char_url, char_headers, ~c"application/json", String.to_charlist(body)}
      :put -> {char_url, char_headers, ~c"application/json", String.to_charlist(body)}
      :patch -> {char_url, char_headers, ~c"application/json", String.to_charlist(body)}
      _ -> {char_url, char_headers}
    end
  end

  defp build_httpc_options(opts) do
    timeout_ms = Keyword.get(opts, :timeout_ms, @default_timeout_ms)
    connect_timeout_ms = Keyword.get(opts, :connect_timeout_ms, @default_connect_timeout_ms)

    [
      timeout: timeout_ms,
      connect_timeout: connect_timeout_ms,
      autoredirect: false
    ]
  end

  defp resolve_host(url, host, opts) do
    case parse_ip_literal(host) do
      {:ok, ip} ->
        {:ok, [ip]}

      :error ->
        dns_resolver = Keyword.get(opts, :dns_resolver, &default_dns_resolver/1)

        case dns_resolver.(host) do
          {:ok, ips} when is_list(ips) and ips != [] ->
            {:ok, ips}

          {:ok, _empty} ->
            block("ssrf_block.dns_resolution_failed", url, host, "host resolution failed", opts)

          {:error, _reason} ->
            block("ssrf_block.dns_resolution_failed", url, host, "host resolution failed", opts)

          _ ->
            block("ssrf_block.dns_resolution_failed", url, host, "host resolution failed", opts)
        end
    end
  end

  defp default_dns_resolver(host) when is_binary(host) do
    host_charlist = String.to_charlist(host)

    ipv4 =
      case :inet.getaddrs(host_charlist, :inet) do
        {:ok, ips} -> ips
        _ -> []
      end

    ipv6 =
      case :inet.getaddrs(host_charlist, :inet6) do
        {:ok, ips} -> ips
        _ -> []
      end

    ips = (ipv4 ++ ipv6) |> Enum.uniq()

    if ips == [] do
      {:error, :nxdomain}
    else
      {:ok, ips}
    end
  end

  defp parse_uri(url, opts) do
    uri = URI.parse(url)

    if uri.scheme in ["http", "https"] and is_binary(uri.host) and String.trim(uri.host) != "" do
      {:ok, uri}
    else
      block("ssrf_block.invalid_url", url, nil, "invalid URL for outbound request", opts)
    end
  end

  defp normalized_host(%URI{host: host}, url, opts) when is_binary(host) do
    host
    |> normalize_host_value()
    |> case do
      nil -> block("ssrf_block.invalid_url", url, nil, "invalid URL host", opts)
      normalized -> {:ok, normalized}
    end
  end

  defp normalized_host(_, url, opts),
    do: block("ssrf_block.invalid_url", url, nil, "invalid URL host", opts)

  defp resolve_redirect_url(base_url, location)
       when is_binary(base_url) and is_binary(location) do
    case URI.parse(location) do
      %URI{scheme: nil} ->
        {:ok, URI.merge(base_url, location) |> URI.to_string()}

      %URI{scheme: scheme} when scheme in ["http", "https"] ->
        {:ok, URI.to_string(URI.parse(location))}

      _ ->
        {:error, :invalid_location}
    end
  rescue
    _ -> {:error, :invalid_location}
  end

  defp location_header(headers) when is_list(headers) do
    Enum.find_value(headers, fn
      {name, value} when is_binary(name) and is_binary(value) ->
        if String.downcase(String.trim(name)) == "location", do: String.trim(value), else: nil

      {name, value} ->
        normalized_name = name |> to_string() |> String.downcase() |> String.trim()
        if normalized_name == "location", do: value |> to_string() |> String.trim(), else: nil

      _ ->
        nil
    end)
  end

  defp block(reason_code, url, host, message, opts) do
    details =
      %{
        "reason_code" => reason_code,
        "host" => host,
        "url" => sanitize_url(url),
        "message" => message
      }
      |> Sanitizer.sanitize()

    emit_blocked_event(reason_code, details, opts)
    {:error, {:blocked, reason_code, details}}
  end

  defp emit_blocked_event(reason_code, details, opts) do
    metadata =
      opts
      |> Keyword.get(:audit_metadata, %{})
      |> normalize_map()
      |> Map.merge(%{
        reason_code: reason_code,
        host: details["host"] || "unknown",
        url: details["url"] || "unknown"
      })

    Events.emit([:openagents_runtime, :tools, :network, :blocked], %{count: 1}, metadata)
  end

  defp transport_opts(opts, host, pinned_ips) do
    [
      timeout_ms: Keyword.get(opts, :timeout_ms, @default_timeout_ms),
      connect_timeout_ms: Keyword.get(opts, :connect_timeout_ms, @default_connect_timeout_ms),
      host: host,
      pinned_ips: Enum.map(pinned_ips, &ip_to_string/1)
    ]
  end

  defp normalize_headers(headers) when is_list(headers) do
    Enum.flat_map(headers, fn
      {name, value} ->
        [{to_string(name), to_string(value)}]

      _ ->
        []
    end)
  end

  defp normalize_host_value(value) do
    value
    |> to_string()
    |> String.trim()
    |> String.downcase()
    |> case do
      "" -> nil
      normalized -> normalized
    end
  rescue
    _ -> nil
  end

  defp host_matches_pattern?(host, pattern) when is_binary(host) and is_binary(pattern) do
    cond do
      String.starts_with?(pattern, "*.") ->
        suffix = String.trim_leading(pattern, "*.")
        host == suffix or String.ends_with?(host, "." <> suffix)

      true ->
        host == pattern
    end
  end

  defp parse_ip_literal(host) do
    with {:ok, parsed} <- :inet.parse_address(String.to_charlist(host)) do
      {:ok, parsed}
    else
      _ -> :error
    end
  end

  defp metadata_ip?({169, 254, 169, 254}), do: true
  defp metadata_ip?(_), do: false

  defp private_or_internal_ip?({10, _, _, _}), do: true
  defp private_or_internal_ip?({127, _, _, _}), do: true
  defp private_or_internal_ip?({169, 254, _, _}), do: true
  defp private_or_internal_ip?({192, 168, _, _}), do: true
  defp private_or_internal_ip?({172, second, _, _}) when second in 16..31, do: true
  defp private_or_internal_ip?({100, second, _, _}) when second in 64..127, do: true
  defp private_or_internal_ip?({0, _, _, _}), do: true

  defp private_or_internal_ip?({segment, _, _, _, _, _, _, _}) when segment in 0xFC00..0xFDFF,
    do: true

  defp private_or_internal_ip?({segment, _, _, _, _, _, _, _}) when segment in 0xFE80..0xFEBF,
    do: true

  defp private_or_internal_ip?({0, 0, 0, 0, 0, 0, 0, 1}), do: true
  defp private_or_internal_ip?({0, 0, 0, 0, 0, 0, 0, 0}), do: true
  defp private_or_internal_ip?(_), do: false

  defp ip_to_string(ip) do
    ip
    |> :inet.ntoa()
    |> to_string()
  end

  defp sanitize_url(nil), do: nil

  defp sanitize_url(url) when is_binary(url) do
    case URI.parse(url) do
      %URI{} = uri ->
        uri
        |> Map.put(:query, nil)
        |> Map.put(:userinfo, nil)
        |> URI.to_string()

      _ ->
        nil
    end
  rescue
    _ -> nil
  end

  defp normalize_map(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {key, value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp normalize_map(_), do: %{}

  defp guard_enabled?(opts) do
    case Keyword.fetch(opts, :guard_enabled) do
      {:ok, value} ->
        value == true

      :error ->
        Application.get_env(:openagents_runtime, :guarded_outbound_http_enabled, true)
    end
  end
end
