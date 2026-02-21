defmodule OpenAgentsRuntime.DS.Strategies.RlmLiteV1 do
  @moduledoc """
  RLM-lite strategy with bounded iterative refinement and trace capture.
  """

  alias OpenAgentsRuntime.DS.Receipts
  alias OpenAgentsRuntime.DS.ToolReplay
  alias OpenAgentsRuntime.DS.Traces

  @default_max_iterations 3
  @max_iterations_cap 8

  @type execution_opt ::
          {:run_id, String.t()}
          | {:max_iterations, pos_integer()}
          | {:tool_replay, map()}
          | {:tool_replay_tasks, [map()]}
          | {:max_replay_items, pos_integer()}
          | {:max_replay_item_chars, pos_integer()}
          | {:max_replay_total_chars, pos_integer()}
          | {:max_inline_trace_bytes, pos_integer()}
          | {:trace_uri_prefix, String.t()}
          | {:output, map() | String.t()}

  @spec id() :: String.t()
  def id, do: "rlm_lite.v1"

  @spec execute(map(), map(), [execution_opt()]) ::
          {:ok, %{output: map(), trace: map(), replay: map()}}
          | {:error, :invalid_iteration_budget}
          | {:error, :invalid_output}
  def execute(signature, input, opts \\ []) when is_map(signature) and is_map(input) do
    with {:ok, max_iterations} <- normalize_iterations(Keyword.get(opts, :max_iterations)),
         {:ok, replay_context} <- replay_context(input, opts),
         {:ok, output} <- resolve_output(signature, input, replay_context, max_iterations, opts) do
      trace_payload =
        build_trace_payload(signature, input, replay_context, output, max_iterations)

      trace =
        Traces.capture(
          Keyword.get(opts, :run_id, "runtime_predict"),
          Map.get(signature, :signature_id) || Map.get(signature, "signature_id") || "unknown",
          trace_payload,
          max_inline_bytes: Keyword.get(opts, :max_inline_trace_bytes, 3_500),
          uri_prefix: Keyword.get(opts, :trace_uri_prefix)
        )

      {:ok, %{output: output, trace: trace, replay: replay_context}}
    end
  end

  defp resolve_output(signature, input, replay_context, max_iterations, opts) do
    case Keyword.fetch(opts, :output) do
      {:ok, %{} = output} ->
        {:ok, stringify_keys(output)}

      {:ok, output} when is_binary(output) ->
        {:ok, %{"summary" => output, "citations" => [], "confidence" => 0.5}}

      {:ok, _invalid} ->
        {:error, :invalid_output}

      :error ->
        iteration_steps = build_iterations(signature, input, replay_context, max_iterations)
        final_step = List.last(iteration_steps) || %{}
        summary_digest = final_step["state_hash"] || Receipts.stable_hash(input)

        output = %{
          "summary" => "RLM summary #{String.slice(summary_digest, 0, 16)}",
          "citations" => Enum.take(Map.get(replay_context, "trace_refs", []), 3),
          "confidence" => confidence(max_iterations, replay_context)
        }

        {:ok, output}
    end
  end

  defp build_trace_payload(signature, input, replay_context, output, max_iterations) do
    iterations = build_iterations(signature, input, replay_context, max_iterations)

    %{
      "strategy_id" => id(),
      "signature_id" => Map.get(signature, :signature_id) || Map.get(signature, "signature_id"),
      "max_iterations" => max_iterations,
      "iterations" => iterations,
      "replay_summary" => Map.get(replay_context, "summary"),
      "replay_window" => Map.get(replay_context, "window", %{}),
      "input_hash" => Receipts.stable_hash(input),
      "output_hash" => Receipts.stable_hash(output)
    }
  end

  defp build_iterations(signature, input, replay_context, max_iterations) do
    signature_id = Map.get(signature, :signature_id) || Map.get(signature, "signature_id")

    Enum.map(1..max_iterations, fn iteration ->
      state_hash =
        Receipts.stable_hash(%{
          signature_id: signature_id,
          iteration: iteration,
          input_hash: Receipts.stable_hash(input),
          replay_hash: Receipts.stable_hash(replay_context)
        })

      %{
        "iteration" => iteration,
        "state_hash" => state_hash,
        "decision" => if(iteration == max_iterations, do: "finalize", else: "refine")
      }
    end)
  end

  defp replay_context(input, opts) do
    cond do
      is_map(Keyword.get(opts, :tool_replay)) ->
        {:ok, stringify_keys(Keyword.get(opts, :tool_replay))}

      is_list(Keyword.get(opts, :tool_replay_tasks)) ->
        replay =
          ToolReplay.build_from_tasks(
            Keyword.get(opts, :tool_replay_tasks, []),
            max_items: Keyword.get(opts, :max_replay_items, 20),
            max_item_chars: Keyword.get(opts, :max_replay_item_chars, 280),
            max_total_chars: Keyword.get(opts, :max_replay_total_chars, 3_500)
          )

        {:ok, replay}

      true ->
        replay =
          input
          |> Map.get("tool_replay")
          |> case do
            %{} = existing ->
              existing

            _ ->
              ToolReplay.build(
                Keyword.get(opts, :run_id, "runtime_predict"),
                max_items: Keyword.get(opts, :max_replay_items, 20),
                max_item_chars: Keyword.get(opts, :max_replay_item_chars, 280),
                max_total_chars: Keyword.get(opts, :max_replay_total_chars, 3_500)
              )
          end

        {:ok, stringify_keys(replay)}
    end
  end

  defp normalize_iterations(nil), do: {:ok, @default_max_iterations}

  defp normalize_iterations(iterations)
       when is_integer(iterations) and iterations > 0 and iterations <= @max_iterations_cap,
       do: {:ok, iterations}

  defp normalize_iterations(_), do: {:error, :invalid_iteration_budget}

  defp confidence(max_iterations, replay_context) do
    base = 0.45 + max_iterations * 0.08

    replay_bonus =
      replay_context |> Map.get("window", %{}) |> Map.get("included_items", 0) |> min(5)

    min(base + replay_bonus * 0.02, 0.95) |> Float.round(2)
  end

  defp stringify_keys(%{} = map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), stringify_keys(value)}
      {key, value} -> {to_string(key), stringify_keys(value)}
    end)
  end

  defp stringify_keys(list) when is_list(list), do: Enum.map(list, &stringify_keys/1)
  defp stringify_keys(value), do: value
end
