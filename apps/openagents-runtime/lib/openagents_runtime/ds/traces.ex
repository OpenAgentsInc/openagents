defmodule OpenAgentsRuntime.DS.Traces do
  @moduledoc """
  Trace capture helpers with pointer-based large artifact handling.
  """

  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.Security.Sanitizer

  @default_inline_bytes 3_500
  @default_uri_prefix "gcs://openagents-runtime-ds-traces"

  @type capture_opt ::
          {:max_inline_bytes, pos_integer()}
          | {:uri_prefix, String.t()}

  @spec pointer(String.t(), String.t()) :: String.t()
  def pointer(run_id, trace_id) when is_binary(run_id) and is_binary(trace_id) do
    "trace:" <> run_id <> ":" <> trace_id
  end

  @spec capture(String.t(), String.t(), map(), [capture_opt()]) :: map()
  def capture(run_id, signature_id, payload, opts \\ [])
      when is_binary(run_id) and is_binary(signature_id) and is_map(payload) do
    max_inline_bytes = Keyword.get(opts, :max_inline_bytes, @default_inline_bytes)
    uri_prefix = Keyword.get(opts, :uri_prefix, @default_uri_prefix)
    payload = Sanitizer.sanitize(payload)

    trace_hash = Receipts.stable_hash(payload)
    trace_id = String.slice(trace_hash, 0, 20)
    trace_ref = pointer(run_id, trace_id)
    encoded = Jason.encode!(payload)

    if byte_size(encoded) <= max_inline_bytes do
      %{
        "trace_id" => trace_id,
        "trace_ref" => trace_ref,
        "trace_hash" => trace_hash,
        "storage" => "inline",
        "artifact_uri" => nil,
        "signature_id" => signature_id,
        "payload" => payload
      }
    else
      artifact_uri = "#{uri_prefix}/#{run_id}/#{trace_id}.json"

      %{
        "trace_id" => trace_id,
        "trace_ref" => trace_ref,
        "trace_hash" => trace_hash,
        "storage" => "external",
        "artifact_uri" => artifact_uri,
        "signature_id" => signature_id,
        "payload" => summarize_payload(payload),
        "offloaded_bytes" => byte_size(encoded)
      }
    end
  end

  defp summarize_payload(payload) do
    %{
      "summary" =>
        payload
        |> Map.take(["strategy_id", "signature_id", "max_iterations"])
        |> Map.put("keys", Map.keys(payload) |> Enum.sort()),
      "hash" => Receipts.stable_hash(payload)
    }
  end
end
