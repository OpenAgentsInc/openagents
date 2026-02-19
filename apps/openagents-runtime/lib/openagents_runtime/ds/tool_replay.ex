defmodule OpenAgentsRuntime.DS.ToolReplay do
  @moduledoc """
  Bounded, deterministic, and redacted tool replay context builder.
  """

  alias OpenAgentsRuntime.Tools.ToolTasks

  @default_max_items 20
  @default_max_item_chars 280
  @default_max_total_chars 3_500
  @redacted "[REDACTED]"

  @sensitive_key_fragments [
    "authorization",
    "cookie",
    "token",
    "secret",
    "password",
    "api_key",
    "apikey",
    "x-api-key",
    "set-cookie"
  ]

  @type replay_context :: %{
          String.t() => term()
        }

  @doc """
  Builds replay context for a run from recent durable tool task records.
  """
  @spec build(String.t(), keyword()) :: replay_context()
  def build(run_id, opts \\ []) when is_binary(run_id) do
    max_items = Keyword.get(opts, :max_items, @default_max_items)

    tasks =
      run_id
      |> ToolTasks.list_for_run(limit: max_items + 1)
      |> sort_tasks()

    build_from_tasks(tasks, opts)
    |> Map.put("run_id", run_id)
  end

  @doc """
  Builds replay context from pre-loaded tool task-like maps.
  """
  @spec build_from_tasks([map()], keyword()) :: replay_context()
  def build_from_tasks(tasks, opts \\ []) when is_list(tasks) do
    max_items = Keyword.get(opts, :max_items, @default_max_items)
    max_item_chars = Keyword.get(opts, :max_item_chars, @default_max_item_chars)
    max_total_chars = Keyword.get(opts, :max_total_chars, @default_max_total_chars)

    {candidate_tasks, truncated_by_limit} = take_bounded(tasks, max_items)

    candidate_items =
      candidate_tasks
      |> Enum.map(&to_replay_item(&1, max_item_chars))

    {items, truncated_by_budget} = apply_total_budget(candidate_items, max_total_chars)

    summary =
      items
      |> Enum.map(&summary_line/1)
      |> Enum.join("\n")
      |> truncate(max_total_chars)

    %{
      "summary" => summary,
      "items" => items,
      "trace_refs" => Enum.map(items, &("tool_task:" <> &1["tool_call_id"])),
      "window" => %{
        "max_items" => max_items,
        "included_items" => length(items),
        "truncated_items" => truncated_by_limit + truncated_by_budget,
        "max_total_chars" => max_total_chars
      }
    }
  end

  @doc """
  Legacy summary helper retained for compatibility.
  """
  @spec summarize([map()]) :: String.t()
  def summarize(tasks) when is_list(tasks) do
    tasks
    |> build_from_tasks(max_items: 8, max_item_chars: 80, max_total_chars: 600)
    |> Map.fetch!("summary")
  end

  defp to_replay_item(task, max_item_chars) do
    task = stringify_keys(task)

    %{
      "tool_call_id" => task["tool_call_id"] || "unknown",
      "tool_name" => task["tool_name"] || "unknown",
      "state" => task["state"] || "unknown",
      "error_class" => task["error_class"],
      "queued_at" => normalize_datetime(task["queued_at"]),
      "terminal_at" => normalize_datetime(terminal_timestamp(task)),
      "input_preview" => task |> Map.get("input", %{}) |> sanitize() |> preview(max_item_chars),
      "output_preview" => task |> Map.get("output") |> sanitize() |> preview(max_item_chars)
    }
  end

  defp apply_total_budget(items, max_total_chars) do
    {included, omitted, _used_chars} =
      Enum.reduce(items, {[], 0, 0}, fn item, {acc, omitted_count, used_chars} ->
        item_size = item |> Jason.encode!() |> byte_size()
        separator_cost = if acc == [], do: 0, else: 1

        if used_chars + item_size + separator_cost <= max_total_chars do
          {[item | acc], omitted_count, used_chars + item_size + separator_cost}
        else
          {acc, omitted_count + 1, used_chars}
        end
      end)

    {Enum.reverse(included), omitted}
  end

  defp take_bounded(tasks, max_items) do
    count = max(max_items, 0)
    sliced = Enum.take(tasks, count)
    truncated = max(length(tasks) - length(sliced), 0)
    {sliced, truncated}
  end

  defp sort_tasks(tasks) do
    Enum.sort_by(tasks, fn task ->
      task = stringify_keys(task)

      {
        normalize_datetime(task["queued_at"] || task["inserted_at"]),
        task["tool_call_id"] || ""
      }
    end)
  end

  defp terminal_timestamp(task) when is_map(task) do
    task["succeeded_at"] || task["failed_at"] || task["canceled_at"] || task["timed_out_at"]
  end

  defp summary_line(item) do
    [
      item["state"],
      item["tool_name"],
      item["tool_call_id"],
      item["error_class"] || "none",
      item["output_preview"] || ""
    ]
    |> Enum.join("|")
  end

  defp preview(nil, _max_chars), do: nil

  defp preview(value, max_chars) do
    value
    |> canonicalize()
    |> inspect(limit: :infinity, printable_limit: :infinity, pretty: false)
    |> truncate(max_chars)
  end

  defp sanitize(%{} = map) do
    map
    |> Enum.map(fn {key, value} ->
      key_string = to_string(key)

      if sensitive_key?(key_string) do
        {key_string, @redacted}
      else
        {key_string, sanitize(value)}
      end
    end)
    |> Enum.into(%{})
  end

  defp sanitize(list) when is_list(list), do: Enum.map(list, &sanitize/1)

  defp sanitize(value) when is_binary(value) do
    value
    |> String.replace(~r/Bearer\s+[A-Za-z0-9\-\._~\+\/=]+/i, "Bearer #{@redacted}")
    |> String.replace(~r/sk-[A-Za-z0-9_\-]+/, @redacted)
    |> String.replace(~r/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/i, @redacted)
  end

  defp sanitize(value), do: value

  defp sensitive_key?(key) do
    normalized = key |> String.downcase() |> String.replace("-", "_")
    Enum.any?(@sensitive_key_fragments, &String.contains?(normalized, &1))
  end

  defp canonicalize(%{} = map) do
    map
    |> Enum.map(fn {key, value} -> {to_string(key), canonicalize(value)} end)
    |> Enum.sort_by(&elem(&1, 0))
  end

  defp canonicalize(list) when is_list(list), do: Enum.map(list, &canonicalize/1)
  defp canonicalize(value), do: value

  defp normalize_datetime(nil), do: nil
  defp normalize_datetime(%DateTime{} = datetime), do: DateTime.to_iso8601(datetime)
  defp normalize_datetime(value) when is_binary(value), do: value
  defp normalize_datetime(value), do: to_string(value)

  defp stringify_keys(%_{} = struct) do
    struct
    |> Map.from_struct()
    |> stringify_keys()
  end

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp stringify_keys(other), do: other

  defp truncate(value, max_chars)
       when is_binary(value) and is_integer(max_chars) and max_chars >= 0 do
    if String.length(value) <= max_chars do
      value
    else
      String.slice(value, 0, max(max_chars - 1, 0)) <> "…"
    end
  end

  defp truncate(value, _max_chars), do: value
end
