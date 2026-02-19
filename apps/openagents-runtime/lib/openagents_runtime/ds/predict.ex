defmodule OpenAgentsRuntime.DS.Predict do
  @moduledoc """
  Predict orchestration for DS signatures with deterministic direct strategy receipts.
  """

  alias OpenAgentsRuntime.DS.PolicyRegistry
  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.DS.Signatures.Catalog
  alias OpenAgentsRuntime.DS.Strategies.DirectV1

  @default_run_id "runtime_predict"
  @default_strategy_id "direct.v1"
  @default_policy_decision "allowed"

  @type run_opt ::
          {:run_id, String.t()}
          | {:strategy_id, String.t()}
          | {:compiled_id, String.t()}
          | {:artifact, map()}
          | {:policy, map()}
          | {:budget, map()}
          | {:started_at, DateTime.t()}
          | {:completed_at, DateTime.t()}
          | {:output, map() | String.t()}

  @spec run(String.t(), map(), [run_opt()]) ::
          {:ok, %{signature_id: String.t(), output: map(), receipt: map()}}
          | {:error, :signature_not_found}
          | {:error, {:unsupported_strategy, String.t()}}
          | {:error, {:artifact_incompatible, term()}}
          | {:error, :invalid_output}
  def run(signature_id, input, opts \\ []) when is_binary(signature_id) and is_map(input) do
    with {:ok, signature} <- Catalog.fetch(signature_id),
         :ok <- validate_strategy(signature, opts),
         {:ok, compiled} <- resolve_compiled(signature_id, opts),
         :ok <- validate_artifact(signature_id, compiled[:artifact]),
         {:ok, output} <- DirectV1.execute(signature, input, opts) do
      started_at = Keyword.get(opts, :started_at, DateTime.utc_now())
      completed_at = Keyword.get(opts, :completed_at, DateTime.utc_now())

      {:ok, signature_hashes} = Catalog.hashes(signature)

      params_hash = Receipts.stable_hash(input)
      output_hash = Receipts.stable_hash(output)
      budget = normalize_budget(input, output, Keyword.get(opts, :budget, %{}))
      policy = normalize_policy(Keyword.get(opts, :policy, %{}))

      receipt =
        Receipts.build_predict(%{
          run_id: Keyword.get(opts, :run_id, @default_run_id),
          signature_id: signature_id,
          strategy_id: strategy_id(signature, opts),
          compiled_id: compiled[:compiled_id],
          schema_hash: signature_hashes.schema_hash,
          prompt_hash: signature_hashes.prompt_hash,
          program_hash: signature_hashes.program_hash,
          params_hash: params_hash,
          output_hash: output_hash,
          policy: policy,
          budget: budget,
          timing: %{
            "started_at" => DateTime.to_iso8601(started_at),
            "completed_at" => DateTime.to_iso8601(completed_at),
            "latency_ms" => max(DateTime.diff(completed_at, started_at, :millisecond), 0)
          },
          catalog_version: Catalog.catalog_version()
        })

      {:ok, %{signature_id: signature_id, output: output, receipt: receipt}}
    else
      {:error, :not_found} -> {:error, :signature_not_found}
      {:error, {:unsupported_strategy, _strategy_id} = reason} -> {:error, reason}
      {:error, {:artifact_incompatible, _reason} = reason} -> {:error, reason}
      {:error, :invalid_output} -> {:error, :invalid_output}
    end
  end

  defp strategy_id(signature, opts) do
    Keyword.get(opts, :strategy_id) ||
      Map.get(signature, :program_template) ||
      Map.get(signature, "program_template") ||
      @default_strategy_id
  end

  defp validate_strategy(signature, opts) do
    strategy = strategy_id(signature, opts)

    if strategy == DirectV1.id() do
      :ok
    else
      {:error, {:unsupported_strategy, strategy}}
    end
  end

  defp resolve_compiled(signature_id, opts) do
    case Keyword.fetch(opts, :compiled_id) do
      {:ok, compiled_id} when is_binary(compiled_id) ->
        {:ok, %{compiled_id: compiled_id, artifact: Keyword.get(opts, :artifact)}}

      _ ->
        case PolicyRegistry.active_artifact(signature_id) do
          {:ok, nil} ->
            {:ok,
             %{
               compiled_id: "catalog:#{signature_id}:direct.v1",
               artifact: Keyword.get(opts, :artifact)
             }}

          {:ok, artifact} when is_map(artifact) ->
            {:ok,
             %{
               compiled_id:
                 artifact[:compiled_id] ||
                   artifact["compiled_id"] ||
                   "catalog:#{signature_id}:direct.v1",
               artifact: artifact
             }}
        end
    end
  end

  defp validate_artifact(_signature_id, nil), do: :ok

  defp validate_artifact(signature_id, artifact) do
    case Catalog.validate_artifact(signature_id, artifact) do
      :ok -> :ok
      {:error, reason} -> {:error, {:artifact_incompatible, reason}}
    end
  end

  defp normalize_policy(policy) when is_map(policy) do
    policy =
      Map.new(policy, fn
        {key, value} when is_atom(key) -> {Atom.to_string(key), value}
        {key, value} -> {to_string(key), value}
      end)

    policy
    |> Map.put_new("decision", @default_policy_decision)
    |> Map.put_new("authorization_mode", "delegated_budget")
  end

  defp normalize_policy(_), do: %{"decision" => @default_policy_decision}

  defp normalize_budget(input, output, budget) when is_map(budget) do
    default_budget = %{
      "input_tokens" => estimate_tokens(input),
      "output_tokens" => estimate_tokens(output),
      "total_tokens" => estimate_tokens(input) + estimate_tokens(output),
      "spent_sats" => 0,
      "reserved_sats" => 0,
      "remaining_sats" => nil
    }

    budget =
      Map.new(budget, fn
        {key, value} when is_atom(key) -> {Atom.to_string(key), value}
        {key, value} -> {to_string(key), value}
      end)

    Map.merge(default_budget, budget)
  end

  defp normalize_budget(input, output, _budget), do: normalize_budget(input, output, %{})

  defp estimate_tokens(value) do
    value
    |> :erlang.term_to_binary()
    |> byte_size()
    |> div(4)
    |> max(1)
  end
end
