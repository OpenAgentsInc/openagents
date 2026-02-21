defmodule OpenAgentsRuntime.DS.Compile.DatasetExporter do
  @moduledoc """
  Deterministic dataset export from DS predict receipts and trace records.
  """

  alias OpenAgentsRuntime.DS.Receipts

  @default_split %{train: 80, holdout: 10, test: 10}
  @required_receipt_fields ~w(signature_id params_hash output_hash strategy_id compiled_id)

  @type export_opt ::
          {:split,
           %{train: non_neg_integer(), holdout: non_neg_integer(), test: non_neg_integer()}}
          | {:split_seed, String.t()}
          | {:dataset_name, String.t()}
          | {:generated_at, DateTime.t() | String.t()}
          | {:max_examples, pos_integer()}

  @spec export([map()]) :: {:ok, map()} | {:error, term()}
  def export(receipts) when is_list(receipts), do: export(receipts, [], [])

  @spec export([map()], [map()]) :: {:ok, map()} | {:error, term()}
  def export(receipts, traces) when is_list(receipts) and is_list(traces),
    do: export(receipts, traces, [])

  @spec export([map()], [map()], [export_opt()]) :: {:ok, map()} | {:error, term()}
  def export(receipts, traces, opts)
      when is_list(receipts) and is_list(traces) and is_list(opts) do
    with :ok <- validate_split(Keyword.get(opts, :split, @default_split)) do
      split = Keyword.get(opts, :split, @default_split)
      split_seed = Keyword.get(opts, :split_seed, "default")
      dataset_name = Keyword.get(opts, :dataset_name, "runtime_ds_dataset")
      max_examples = Keyword.get(opts, :max_examples)
      trace_lookup = build_trace_lookup(traces)
      generated_at = normalize_timestamp(Keyword.get(opts, :generated_at, DateTime.utc_now()))

      {examples, skipped} =
        receipts
        |> Enum.map(&normalize_map/1)
        |> Enum.reduce({[], []}, fn receipt, {acc_examples, acc_skipped} ->
          case to_example(receipt, trace_lookup) do
            {:ok, example} ->
              {[example | acc_examples], acc_skipped}

            {:error, reason} ->
              {acc_examples, [%{receipt: receipt, reason: reason} | acc_skipped]}
          end
        end)

      examples =
        examples
        |> Enum.sort_by(& &1["example_id"])
        |> maybe_limit_examples(max_examples)

      split_examples = split_examples(examples, split, split_seed)
      split_ids = split_ids(split_examples)

      provenance = %{
        "dataset_name" => dataset_name,
        "split_seed" => split_seed,
        "split" => split,
        "receipt_count" => length(receipts),
        "trace_count" => length(traces),
        "generated_at" => generated_at
      }

      dataset_hash =
        Receipts.stable_hash(%{
          split: split,
          split_seed: split_seed,
          ids: split_ids
        })

      job_hash =
        Receipts.stable_hash(%{
          dataset_name: dataset_name,
          dataset_hash: dataset_hash,
          provenance: provenance
        })

      {:ok,
       %{
         dataset_name: dataset_name,
         dataset_hash: dataset_hash,
         job_hash: job_hash,
         counts: %{
           total: length(examples),
           train: length(split_examples.train),
           holdout: length(split_examples.holdout),
           test: length(split_examples.test),
           skipped: length(skipped)
         },
         split: split,
         split_seed: split_seed,
         splits: split_examples,
         provenance: provenance,
         skipped: skipped
       }}
    end
  end

  defp split_examples(examples, split, split_seed) do
    train_cutoff = split.train
    holdout_cutoff = split.train + split.holdout

    Enum.reduce(examples, %{train: [], holdout: [], test: []}, fn example, acc ->
      bucket = example_bucket(example["example_id"], split_seed)

      cond do
        bucket < train_cutoff ->
          %{acc | train: [example | acc.train]}

        bucket < holdout_cutoff ->
          %{acc | holdout: [example | acc.holdout]}

        true ->
          %{acc | test: [example | acc.test]}
      end
    end)
    |> Map.update!(:train, &Enum.reverse/1)
    |> Map.update!(:holdout, &Enum.reverse/1)
    |> Map.update!(:test, &Enum.reverse/1)
  end

  defp split_ids(split_examples) do
    %{
      train: Enum.map(split_examples.train, & &1["example_id"]),
      holdout: Enum.map(split_examples.holdout, & &1["example_id"]),
      test: Enum.map(split_examples.test, & &1["example_id"])
    }
  end

  defp to_example(receipt, trace_lookup) do
    missing =
      @required_receipt_fields
      |> Enum.reject(&present?(receipt, &1))

    if missing != [] do
      {:error, {:missing_fields, missing}}
    else
      trace_ref = receipt["trace_ref"]
      trace = trace_lookup[trace_ref]

      example = %{
        "example_id" => example_id(receipt),
        "receipt_id" => receipt["receipt_id"] || "missing_receipt_id",
        "signature_id" => receipt["signature_id"],
        "strategy_id" => receipt["strategy_id"],
        "compiled_id" => receipt["compiled_id"],
        "params_hash" => receipt["params_hash"],
        "output_hash" => receipt["output_hash"],
        "policy" => normalize_map(receipt["policy"] || %{}),
        "budget" => normalize_map(receipt["budget"] || %{}),
        "timing" => normalize_map(receipt["timing"] || %{}),
        "trace_ref" => trace_ref,
        "trace_hash" => receipt["trace_hash"] || (trace && trace["trace_hash"]),
        "trace_storage" => receipt["trace_storage"] || (trace && trace["storage"]),
        "trace_artifact_uri" => receipt["trace_artifact_uri"] || (trace && trace["artifact_uri"]),
        "trace_summary" => trace_summary(trace)
      }

      {:ok, example}
    end
  end

  defp trace_summary(nil), do: nil

  defp trace_summary(trace) do
    payload = normalize_map(trace["payload"] || %{})

    payload["summary"] ||
      payload["replay_summary"] ||
      if(map_size(payload) == 0,
        do: nil,
        else: Map.take(payload, ["strategy_id", "signature_id"])
      )
  end

  defp example_id(receipt) do
    receipt["receipt_id"] ||
      Receipts.stable_hash(%{
        signature_id: receipt["signature_id"],
        params_hash: receipt["params_hash"],
        output_hash: receipt["output_hash"],
        strategy_id: receipt["strategy_id"],
        compiled_id: receipt["compiled_id"]
      })
  end

  defp present?(receipt, key) do
    value = receipt[key]
    not is_nil(value) and value != ""
  end

  defp build_trace_lookup(traces) do
    traces
    |> Enum.map(&normalize_map/1)
    |> Enum.reduce(%{}, fn trace, acc ->
      ref = trace["trace_ref"]

      if is_binary(ref) and ref != "" do
        Map.put(acc, ref, trace)
      else
        acc
      end
    end)
  end

  defp maybe_limit_examples(examples, nil), do: examples

  defp maybe_limit_examples(examples, max_examples)
       when is_integer(max_examples) and max_examples > 0 do
    Enum.take(examples, max_examples)
  end

  defp maybe_limit_examples(examples, _), do: examples

  defp example_bucket(example_id, split_seed) do
    "#{split_seed}|#{example_id}"
    |> Receipts.stable_hash()
    |> String.slice(0, 8)
    |> Integer.parse(16)
    |> case do
      {value, _} -> rem(value, 100)
      :error -> 0
    end
  end

  defp validate_split(split) when is_map(split) do
    train = split[:train] || split["train"] || 0
    holdout = split[:holdout] || split["holdout"] || 0
    test = split[:test] || split["test"] || 0

    cond do
      not is_integer(train) or not is_integer(holdout) or not is_integer(test) ->
        {:error, :invalid_split}

      train < 0 or holdout < 0 or test < 0 ->
        {:error, :invalid_split}

      train + holdout + test != 100 ->
        {:error, :invalid_split}

      true ->
        :ok
    end
  end

  defp validate_split(_), do: {:error, :invalid_split}

  defp normalize_timestamp(%DateTime{} = datetime), do: DateTime.to_iso8601(datetime)
  defp normalize_timestamp(value) when is_binary(value), do: value
  defp normalize_timestamp(_), do: DateTime.to_iso8601(DateTime.utc_now())

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
end
