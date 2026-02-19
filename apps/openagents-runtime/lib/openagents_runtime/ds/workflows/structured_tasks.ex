defmodule OpenAgentsRuntime.DS.Workflows.StructuredTasks do
  @moduledoc """
  DS structured workflow runner for OpenClaw-style `llm-task` and map/reduce subflows.

  Workflows execute through DS signatures/strategies and produce replayable receipts
  with step-level trace linkage.
  """

  alias OpenAgentsRuntime.DS.Predict
  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.DS.Signatures.Catalog

  @structured_task_signature "@openagents/autopilot/workflow/StructuredTask.v1"
  @timeline_map_item_signature "@openagents/autopilot/workflow/TimelineMapItem.v1"
  @timeline_reduce_signature "@openagents/autopilot/workflow/TimelineMapReduce.v1"

  @default_step_cost_sats 1
  @default_remaining_sats 100
  @default_max_steps 16
  @default_max_map_items 10

  @type run_opt ::
          {:run_id, String.t()}
          | {:budget, map()}
          | {:policy, map()}
          | {:policy_context, map()}
          | {:trace_uri_prefix, String.t()}
          | {:max_iterations, pos_integer()}
          | {:map_strategy_id, String.t()}
          | {:reduce_strategy_id, String.t()}
          | {:strategy_id, String.t()}
          | {:max_steps, pos_integer()}
          | {:max_map_items, pos_integer()}
          | {:step_cost_sats, pos_integer()}

  @type run_result :: %{
          required(String.t()) => term()
        }

  @spec signature_ids() :: %{required(atom()) => String.t()}
  def signature_ids do
    %{
      structured_task: @structured_task_signature,
      timeline_map_item: @timeline_map_item_signature,
      timeline_reduce: @timeline_reduce_signature
    }
  end

  @spec run(String.t(), map(), [run_opt()]) ::
          {:ok, run_result()}
          | {:error, :unsupported_workflow | :budget_exhausted | :signature_not_found}
          | {:error, {:schema_violation, String.t()}}
          | {:error, {:step_failed, String.t(), term()}}
  def run(workflow_id, input, opts \\ [])

  def run("llm_task.v1", input, opts) when is_map(input) do
    run_llm_task(input, opts)
  end

  def run("timeline_map_reduce.v1", input, opts) when is_map(input) do
    run_timeline_map_reduce(input, opts)
  end

  def run(_workflow_id, _input, _opts), do: {:error, :unsupported_workflow}

  defp run_llm_task(input, opts) do
    run_id = Keyword.get(opts, :run_id, "workflow_llm_task")
    budget_before = init_budget(opts)

    with {:ok, signature} <- fetch_signature(@structured_task_signature),
         :ok <- validate_schema(input, signature.input_schema, "input"),
         {:ok, budget_after} <- consume_budget(budget_before),
         {:ok, step} <-
           run_step(
             "task",
             @structured_task_signature,
             input,
             budget_after,
             opts,
             strategy_id: Keyword.get(opts, :strategy_id, "direct.v1")
           ),
         :ok <- validate_schema(step["output"], signature.output_schema, "output") do
      {:ok,
       build_workflow_result(
         "llm_task.v1",
         run_id,
         [step],
         step["output"],
         budget_before,
         budget_after
       )}
    end
  end

  defp run_timeline_map_reduce(input, opts) do
    run_id = Keyword.get(opts, :run_id, "workflow_timeline_map_reduce")
    budget_before = init_budget(opts)

    map_reduce_input_schema = %{
      "query" => "string",
      "items" => ["map"]
    }

    with {:ok, map_signature} <- fetch_signature(@timeline_map_item_signature),
         {:ok, reduce_signature} <- fetch_signature(@timeline_reduce_signature),
         :ok <- validate_schema(input, map_reduce_input_schema, "input"),
         {:ok, map_steps, budget_after_map} <-
           run_map_steps(input, map_signature, budget_before, opts),
         {:ok, budget_before_reduce} <- consume_budget(budget_after_map),
         {:ok, reduce_step} <-
           run_reduce_step(
             input,
             map_steps,
             reduce_signature,
             budget_before_reduce,
             opts
           ),
         :ok <- validate_schema(reduce_step["output"], reduce_signature.output_schema, "output") do
      steps = map_steps ++ [reduce_step]

      {:ok,
       build_workflow_result(
         "timeline_map_reduce.v1",
         run_id,
         steps,
         reduce_step["output"],
         budget_before,
         budget_before_reduce
       )}
    end
  end

  defp run_map_steps(input, map_signature, budget, opts) do
    query = input["query"]
    items = input["items"] |> List.wrap() |> Enum.take(budget["max_map_items"])
    strategy_id = Keyword.get(opts, :map_strategy_id, "direct.v1")

    Enum.with_index(items, 1)
    |> Enum.reduce_while({:ok, [], budget}, fn {item, index}, {:ok, acc, budget_state} ->
      step_id = "map_#{index}"

      with {:ok, next_budget} <- consume_budget(budget_state),
           map_input <- %{"query" => query, "item" => stringify_keys(item), "item_index" => index},
           :ok <- validate_schema(map_input, map_signature.input_schema, "input.#{step_id}"),
           {:ok, step} <-
             run_step(
               step_id,
               @timeline_map_item_signature,
               map_input,
               next_budget,
               opts,
               strategy_id: strategy_id
             ),
           :ok <-
             validate_schema(step["output"], map_signature.output_schema, "output.#{step_id}") do
        {:cont, {:ok, acc ++ [step], next_budget}}
      else
        {:error, reason} ->
          {:halt, {:error, {:step_failed, step_id, reason}}}
      end
    end)
  end

  defp run_reduce_step(input, map_steps, reduce_signature, budget, opts) do
    mapped_outputs = Enum.map(map_steps, & &1["output"])

    reduce_input = %{
      "query" => input["query"],
      "mapped_items" => mapped_outputs
    }

    reduce_output = build_reduce_output(input["query"], mapped_outputs)
    reduce_strategy_id = Keyword.get(opts, :reduce_strategy_id, "rlm_lite.v1")

    with :ok <- validate_schema(reduce_input, reduce_signature.input_schema, "input.reduce"),
         {:ok, step} <-
           run_step(
             "reduce",
             @timeline_reduce_signature,
             reduce_input,
             budget,
             opts,
             strategy_id: reduce_strategy_id,
             output: reduce_output
           ) do
      {:ok, step}
    else
      {:error, reason} ->
        {:error, {:step_failed, "reduce", reason}}
    end
  end

  defp run_step(step_id, signature_id, input, budget, opts, extra_opts) do
    run_id = Keyword.get(opts, :run_id, "workflow")
    step_run_id = "#{run_id}:#{step_id}"

    predict_opts =
      [
        run_id: step_run_id,
        strategy_id: Keyword.get(extra_opts, :strategy_id),
        output: Keyword.get(extra_opts, :output),
        budget: budget_for_predict(budget),
        policy: normalize_policy(Keyword.get(opts, :policy, %{})),
        policy_context: stringify_keys(Keyword.get(opts, :policy_context, %{})),
        max_iterations: Keyword.get(opts, :max_iterations),
        trace_uri_prefix: Keyword.get(opts, :trace_uri_prefix)
      ]
      |> Enum.reject(fn
        {_key, nil} -> true
        _ -> false
      end)

    case Predict.run(signature_id, stringify_keys(input), predict_opts) do
      {:ok, result} ->
        step = %{
          "step_id" => step_id,
          "signature_id" => signature_id,
          "strategy_id" => result.receipt.strategy_id,
          "output" => stringify_keys(result.output),
          "receipt" => stringify_keys(result.receipt),
          "trace" => stringify_keys(result[:trace])
        }

        {:ok, step}

      {:error, reason} ->
        {:error, reason}
    end
  end

  defp build_workflow_result(workflow_id, run_id, steps, output, budget_before, budget_after) do
    step_receipts = Enum.map(steps, & &1["receipt"])

    trace_refs =
      step_receipts
      |> Enum.map(&Map.get(&1, "trace_ref"))
      |> Enum.reject(&is_nil/1)

    trace_artifact_uris =
      step_receipts
      |> Enum.map(&Map.get(&1, "trace_artifact_uri"))
      |> Enum.reject(&is_nil/1)

    workflow_receipt =
      %{
        "workflow_id" => workflow_id,
        "run_id" => run_id,
        "status" => "succeeded",
        "step_count" => length(steps),
        "step_receipt_ids" => Enum.map(step_receipts, &Map.get(&1, "receipt_id")),
        "strategy_ids" => Enum.map(step_receipts, &Map.get(&1, "strategy_id")),
        "policy_decisions" =>
          Enum.map(step_receipts, fn receipt ->
            receipt |> Map.get("policy", %{}) |> Map.get("decision")
          end),
        "trace_refs" => trace_refs,
        "trace_artifact_uris" => trace_artifact_uris,
        "budget_before" => budget_view(budget_before),
        "budget_after" => budget_view(budget_after),
        "output_hash" => Receipts.stable_hash(output)
      }
      |> Map.put(
        "replay_hash",
        Receipts.stable_hash(%{
          workflow_id: workflow_id,
          run_id: run_id,
          step_receipt_ids: Enum.map(step_receipts, &Map.get(&1, "receipt_id")),
          output_hash: Receipts.stable_hash(output),
          budget_after: budget_view(budget_after)
        })
      )

    %{
      "workflow_id" => workflow_id,
      "run_id" => run_id,
      "output" => output,
      "receipt" => workflow_receipt,
      "step_results" =>
        Enum.map(steps, fn step ->
          Map.take(step, ["step_id", "signature_id", "strategy_id", "output", "receipt"])
        end),
      "trace_links" => %{
        "trace_refs" => trace_refs,
        "trace_artifact_uris" => trace_artifact_uris
      }
    }
  end

  defp build_reduce_output(query, mapped_outputs) do
    highlights =
      mapped_outputs
      |> Enum.map(fn output -> output["summary"] end)
      |> Enum.filter(&is_binary/1)
      |> Enum.take(4)

    %{
      "summary" => "Reduced #{length(mapped_outputs)} mapped items for query #{query}",
      "highlights" => highlights,
      "item_count" => length(mapped_outputs),
      "confidence" => reduce_confidence(mapped_outputs)
    }
  end

  defp reduce_confidence(mapped_outputs) do
    base = 0.48 + length(mapped_outputs) * 0.03
    min(Float.round(base, 2), 0.93)
  end

  defp init_budget(opts) do
    budget = stringify_keys(Keyword.get(opts, :budget, %{}))

    %{
      "remaining_sats" =>
        normalize_non_negative_int(budget["remaining_sats"], @default_remaining_sats),
      "spent_sats" => normalize_non_negative_int(budget["spent_sats"], 0),
      "reserved_sats" => normalize_non_negative_int(budget["reserved_sats"], 0),
      "max_steps" =>
        normalize_positive_int(
          Keyword.get(opts, :max_steps, budget["max_steps"]),
          @default_max_steps
        ),
      "max_map_items" =>
        normalize_positive_int(
          Keyword.get(opts, :max_map_items, budget["max_map_items"]),
          @default_max_map_items
        ),
      "step_cost_sats" =>
        normalize_positive_int(
          Keyword.get(opts, :step_cost_sats, budget["step_cost_sats"]),
          @default_step_cost_sats
        ),
      "steps_used" => 0
    }
  end

  defp consume_budget(budget) do
    next_steps = budget["steps_used"] + 1
    step_cost = budget["step_cost_sats"]
    remaining = budget["remaining_sats"]

    cond do
      next_steps > budget["max_steps"] ->
        {:error, :budget_exhausted}

      remaining < step_cost ->
        {:error, :budget_exhausted}

      true ->
        {:ok,
         %{
           budget
           | "steps_used" => next_steps,
             "spent_sats" => budget["spent_sats"] + step_cost,
             "remaining_sats" => max(remaining - step_cost, 0)
         }}
    end
  end

  defp budget_for_predict(budget) do
    %{
      "spent_sats" => budget["spent_sats"],
      "reserved_sats" => budget["reserved_sats"],
      "remaining_sats" => budget["remaining_sats"]
    }
  end

  defp budget_view(budget) do
    %{
      "spent_sats" => budget["spent_sats"],
      "reserved_sats" => budget["reserved_sats"],
      "remaining_sats" => budget["remaining_sats"],
      "steps_used" => budget["steps_used"],
      "max_steps" => budget["max_steps"],
      "max_map_items" => budget["max_map_items"]
    }
  end

  defp normalize_policy(policy) do
    stringify_keys(policy)
    |> Map.put_new("authorization_id", "auth_workflow")
    |> Map.put_new("authorization_mode", "delegated_budget")
    |> Map.put_new("decision", "allowed")
    |> Map.put_new("settlement_boundary", false)
  end

  defp fetch_signature(signature_id) do
    case Catalog.fetch(signature_id) do
      {:ok, signature} -> {:ok, signature}
      {:error, :not_found} -> {:error, :signature_not_found}
    end
  end

  defp validate_schema(value, schema, path) do
    case validate_schema_value(value, schema, path) do
      :ok -> :ok
      {:error, reason} -> {:error, {:schema_violation, reason}}
    end
  end

  defp validate_schema_value(value, schema, path) when is_binary(schema) do
    cond do
      schema == "string" and is_binary(value) ->
        :ok

      schema == "number" and (is_integer(value) or is_float(value)) ->
        :ok

      schema == "map" and is_map(value) ->
        :ok

      schema == "boolean" and is_boolean(value) ->
        :ok

      true ->
        {:error, "#{path} expected #{schema}"}
    end
  end

  defp validate_schema_value(value, [inner], path) do
    if is_list(value) do
      value
      |> Enum.with_index()
      |> Enum.reduce_while(:ok, fn {item, index}, _acc ->
        case validate_schema_value(item, inner, "#{path}[#{index}]") do
          :ok -> {:cont, :ok}
          {:error, reason} -> {:halt, {:error, reason}}
        end
      end)
    else
      {:error, "#{path} expected array"}
    end
  end

  defp validate_schema_value(value, schema, path) when is_map(schema) do
    if is_map(value) do
      Enum.reduce_while(schema, :ok, fn {key, inner_schema}, _acc ->
        key = to_string(key)

        if Map.has_key?(value, key) do
          case validate_schema_value(Map.get(value, key), inner_schema, "#{path}.#{key}") do
            :ok -> {:cont, :ok}
            {:error, reason} -> {:halt, {:error, reason}}
          end
        else
          {:halt, {:error, "#{path}.#{key} missing required field"}}
        end
      end)
    else
      {:error, "#{path} expected object"}
    end
  end

  defp validate_schema_value(_value, _schema, path),
    do: {:error, "#{path} has invalid schema contract"}

  defp stringify_keys(%{} = map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), stringify_keys(value)}
      {key, value} -> {to_string(key), stringify_keys(value)}
    end)
  end

  defp stringify_keys(list) when is_list(list), do: Enum.map(list, &stringify_keys/1)
  defp stringify_keys(value), do: value

  defp normalize_non_negative_int(value, fallback) do
    value = normalize_int(value, fallback)
    if value < 0, do: fallback, else: value
  end

  defp normalize_positive_int(value, fallback) do
    value = normalize_int(value, fallback)
    if value > 0, do: value, else: fallback
  end

  defp normalize_int(value, _fallback) when is_integer(value), do: value
  defp normalize_int(value, _fallback) when is_float(value), do: trunc(value)

  defp normalize_int(value, fallback) when is_binary(value) do
    case Integer.parse(String.trim(value)) do
      {parsed, _rest} -> parsed
      :error -> fallback
    end
  end

  defp normalize_int(_value, fallback), do: fallback
end
