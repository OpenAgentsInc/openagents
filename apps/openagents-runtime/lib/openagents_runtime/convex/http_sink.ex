defmodule OpenAgentsRuntime.Convex.HttpSink do
  @moduledoc """
  Convex sink implementation backed by the Convex `/api/mutation` HTTP API.
  """

  @behaviour OpenAgentsRuntime.Convex.Sink

  @default_timeout_ms 2_500
  @default_run_summary_mutation_path "runtime:upsertRunSummary"
  @default_codex_worker_summary_mutation_path "runtime:upsertCodexWorkerSummary"

  @impl true
  def upsert_run_summary(document_id, summary, opts) do
    mutation_path =
      Keyword.get(opts, :run_summary_mutation_path) ||
        Application.get_env(:openagents_runtime, :convex_http_sink, [])
        |> Keyword.get(:run_summary_mutation_path, @default_run_summary_mutation_path)

    mutate(mutation_path, document_id, summary, opts)
  end

  @impl true
  def upsert_codex_worker_summary(document_id, summary, opts) do
    mutation_path =
      Keyword.get(opts, :codex_worker_summary_mutation_path) ||
        Application.get_env(:openagents_runtime, :convex_http_sink, [])
        |> Keyword.get(
          :codex_worker_summary_mutation_path,
          @default_codex_worker_summary_mutation_path
        )

    mutate(mutation_path, document_id, summary, opts)
  end

  defp mutate(mutation_path, document_id, summary, opts) do
    with {:ok, base_url} <- base_url(opts),
         {:ok, body} <- build_body(mutation_path, document_id, summary),
         {:ok, headers} <- headers(opts),
         :ok <- ensure_httpc_started(),
         {:ok, status, response_body} <- post("#{base_url}/api/mutation", headers, body, opts),
         :ok <- decode_response(status, response_body) do
      :ok
    end
  end

  defp base_url(opts) do
    value =
      Keyword.get(opts, :base_url) ||
        Application.get_env(:openagents_runtime, :convex_http_sink, []) |> Keyword.get(:base_url)

    case normalize_url(value) do
      nil -> {:error, :missing_base_url}
      url -> {:ok, url}
    end
  end

  defp headers(opts) do
    admin_key =
      Keyword.get(opts, :admin_key) ||
        Application.get_env(:openagents_runtime, :convex_http_sink, []) |> Keyword.get(:admin_key)

    headers = [{"content-type", "application/json"}]

    case normalize_string(admin_key) do
      nil ->
        {:ok, headers}

      key ->
        {:ok, [{"authorization", "Convex #{key}"} | headers]}
    end
  end

  defp build_body(mutation_path, document_id, summary) do
    Jason.encode(%{
      "path" => mutation_path,
      "format" => "convex_encoded_json",
      "args" => [
        %{
          "document_id" => document_id,
          "summary" => summary
        }
      ]
    })
  end

  defp ensure_httpc_started do
    _ = :inets.start()
    _ = :ssl.start()
    :ok
  end

  defp post(url, headers, body, opts) do
    timeout_ms = timeout_ms(opts)
    request = {String.to_charlist(url), charlist_headers(headers), ~c"application/json", body}
    http_options = [timeout: timeout_ms, connect_timeout: timeout_ms]
    request_options = [body_format: :binary]

    case :httpc.request(:post, request, http_options, request_options) do
      {:ok, {{_version, status, _reason_phrase}, _response_headers, response_body}} ->
        {:ok, status, IO.iodata_to_binary(response_body)}

      {:error, reason} ->
        {:error, {:http_request_failed, reason}}
    end
  end

  defp decode_response(status, response_body) when status >= 200 and status < 300 do
    case Jason.decode(response_body) do
      {:ok, %{"status" => "success"}} ->
        :ok

      {:ok, %{"status" => "error", "errorMessage" => message}} when is_binary(message) ->
        {:error, {:convex_error, message}}

      {:ok, _response} ->
        {:error, {:invalid_response, response_body}}

      {:error, _reason} ->
        {:error, {:invalid_json, response_body}}
    end
  end

  defp decode_response(status, response_body) do
    {:error, {:http_status, status, response_body}}
  end

  defp timeout_ms(opts) do
    value =
      Keyword.get(opts, :timeout_ms) ||
        Application.get_env(:openagents_runtime, :convex_http_sink, [])
        |> Keyword.get(:request_timeout_ms, @default_timeout_ms)

    if is_integer(value) and value > 0, do: value, else: @default_timeout_ms
  end

  defp charlist_headers(headers) do
    Enum.map(headers, fn {name, value} ->
      {String.to_charlist(name), String.to_charlist(value)}
    end)
  end

  defp normalize_string(value) when is_binary(value) do
    trimmed = String.trim(value)
    if trimmed == "", do: nil, else: trimmed
  end

  defp normalize_string(_), do: nil

  defp normalize_url(value) do
    value
    |> normalize_string()
    |> case do
      nil -> nil
      trimmed -> String.trim_trailing(trimmed, "/")
    end
  end
end
