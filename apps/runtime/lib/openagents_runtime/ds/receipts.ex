defmodule OpenAgentsRuntime.DS.Receipts do
  @moduledoc """
  Deterministic receipt helpers for DS predict/tool execution.
  """

  @type predict_receipt :: %{
          required(:receipt_id) => String.t(),
          required(:run_id) => String.t(),
          required(:signature_id) => String.t(),
          required(:strategy_id) => String.t(),
          required(:compiled_id) => String.t(),
          required(:schema_hash) => String.t(),
          required(:prompt_hash) => String.t(),
          required(:program_hash) => String.t(),
          required(:params_hash) => String.t(),
          required(:output_hash) => String.t(),
          required(:policy) => map(),
          required(:budget) => map(),
          required(:timing) => map(),
          required(:catalog_version) => pos_integer(),
          optional(:trace_ref) => String.t() | nil,
          optional(:trace_hash) => String.t() | nil,
          optional(:trace_storage) => String.t() | nil,
          optional(:trace_artifact_uri) => String.t() | nil
        }

  @spec build_predict(map()) :: predict_receipt()
  def build_predict(attrs) when is_map(attrs) do
    run_id = attrs[:run_id] || attrs["run_id"] || "unknown"
    signature_id = attrs[:signature_id] || attrs["signature_id"] || "unknown"
    strategy_id = attrs[:strategy_id] || attrs["strategy_id"] || "unknown"
    compiled_id = attrs[:compiled_id] || attrs["compiled_id"] || "catalog_default"

    schema_hash = attrs[:schema_hash] || attrs["schema_hash"] || stable_hash(%{})
    prompt_hash = attrs[:prompt_hash] || attrs["prompt_hash"] || stable_hash("")
    program_hash = attrs[:program_hash] || attrs["program_hash"] || stable_hash("")
    params_hash = attrs[:params_hash] || attrs["params_hash"] || stable_hash(%{})
    output_hash = attrs[:output_hash] || attrs["output_hash"] || stable_hash(%{})
    policy = normalize_map(attrs[:policy] || attrs["policy"] || %{})
    budget = normalize_map(attrs[:budget] || attrs["budget"] || %{})
    timing = normalize_map(attrs[:timing] || attrs["timing"] || %{})
    catalog_version = attrs[:catalog_version] || attrs["catalog_version"] || 1
    trace_ref = attrs[:trace_ref] || attrs["trace_ref"]
    trace_hash = attrs[:trace_hash] || attrs["trace_hash"]
    trace_storage = attrs[:trace_storage] || attrs["trace_storage"]
    trace_artifact_uri = attrs[:trace_artifact_uri] || attrs["trace_artifact_uri"]

    fingerprint = %{
      run_id: run_id,
      signature_id: signature_id,
      strategy_id: strategy_id,
      compiled_id: compiled_id,
      schema_hash: schema_hash,
      prompt_hash: prompt_hash,
      program_hash: program_hash,
      params_hash: params_hash,
      output_hash: output_hash,
      policy: policy,
      budget: budget,
      trace_ref: trace_ref,
      trace_hash: trace_hash
    }

    %{
      receipt_id: "pred_" <> String.slice(stable_hash(fingerprint), 0, 24),
      run_id: run_id,
      signature_id: signature_id,
      strategy_id: strategy_id,
      compiled_id: compiled_id,
      schema_hash: schema_hash,
      prompt_hash: prompt_hash,
      program_hash: program_hash,
      params_hash: params_hash,
      output_hash: output_hash,
      policy: policy,
      budget: budget,
      timing: timing,
      catalog_version: catalog_version
    }
    |> maybe_put(:trace_ref, trace_ref)
    |> maybe_put(:trace_hash, trace_hash)
    |> maybe_put(:trace_storage, trace_storage)
    |> maybe_put(:trace_artifact_uri, trace_artifact_uri)
  end

  @spec stable_hash(term()) :: String.t()
  def stable_hash(value) do
    value
    |> canonicalize()
    |> :erlang.term_to_binary()
    |> then(&:crypto.hash(:sha256, &1))
    |> Base.encode16(case: :lower)
  end

  defp canonicalize(%{} = map) do
    map
    |> Enum.map(fn {key, value} -> {to_string(key), canonicalize(value)} end)
    |> Enum.sort_by(&elem(&1, 0))
  end

  defp canonicalize(list) when is_list(list), do: Enum.map(list, &canonicalize/1)
  defp canonicalize(value), do: value

  defp normalize_map(%{} = map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), normalize_value(value)}
      {key, value} -> {to_string(key), normalize_value(value)}
    end)
  end

  defp normalize_map(_), do: %{}

  defp normalize_value(%{} = value), do: normalize_map(value)
  defp normalize_value(list) when is_list(list), do: Enum.map(list, &normalize_value/1)
  defp normalize_value(value), do: value

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)
end
