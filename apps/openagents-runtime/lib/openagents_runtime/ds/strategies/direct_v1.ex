defmodule OpenAgentsRuntime.DS.Strategies.DirectV1 do
  @moduledoc """
  Direct strategy implementation and helper values.
  """

  @type execution_opt :: {:output, map() | String.t()}

  @spec id() :: String.t()
  def id, do: "direct.v1"

  @spec execute(map(), map(), [execution_opt()]) :: {:ok, map()} | {:error, :invalid_output}
  def execute(signature, input, opts \\ []) when is_map(signature) and is_map(input) do
    case Keyword.fetch(opts, :output) do
      {:ok, output} ->
        normalize_output(output)

      :error ->
        {:ok, default_output(signature, input)}
    end
  end

  defp normalize_output(%{} = output), do: {:ok, stringify_keys(output)}
  defp normalize_output(output) when is_binary(output), do: {:ok, %{"text" => output}}
  defp normalize_output(_output), do: {:error, :invalid_output}

  defp default_output(signature, input) do
    name = Map.get(signature, :name) || Map.get(signature, "name")

    case name do
      "SelectTool" ->
        %{
          "tool_name" => first_tool_name(input),
          "arguments" => input |> Map.get("arguments", %{}) |> stringify_keys(),
          "confidence" => 0.5
        }

      "RecapThread" ->
        messages = Map.get(input, "messages", [])

        %{
          "summary" => "Recap generated from #{length(messages)} messages.",
          "action_items" => Enum.take(extract_message_lines(messages), 3)
        }

      "SummarizeThread" ->
        summary = input |> Map.get("timeline_window", %{}) |> inspect(limit: 20)

        %{
          "summary" => "Summary: #{summary}",
          "citations" => Enum.take(Map.get(input, "citations", []), 3),
          "confidence" => 0.5
        }

      "StructuredTask" ->
        task = Map.get(input, "task", %{}) |> stringify_keys()
        tools = Map.get(input, "tools", []) |> List.wrap()

        context_keys =
          case Map.get(input, "context", %{}) |> stringify_keys() do
            %{} = context -> Map.keys(context) |> Enum.sort()
            _ -> []
          end

        %{
          "status" => "completed",
          "result" => %{
            "task_id" => task["id"] || "task_unknown",
            "objective" => task["objective"] || "objective_unknown",
            "context_keys" => context_keys
          },
          "next_actions" =>
            tools
            |> Enum.flat_map(fn
              %{"name" => name} when is_binary(name) -> [name]
              %{name: name} when is_binary(name) -> [name]
              _ -> []
            end)
            |> Enum.uniq()
            |> Enum.take(3),
          "confidence" => 0.62
        }

      "TimelineMapItem" ->
        item = Map.get(input, "item", %{}) |> stringify_keys()
        item_index = Map.get(input, "item_index", 0)

        %{
          "item_index" => normalize_number(item_index),
          "summary" => "Timeline item #{item_index}: #{item_digest(item)}",
          "signals" => item |> Map.keys() |> Enum.sort() |> Enum.take(3),
          "confidence" => 0.58
        }

      "TimelineMapReduce" ->
        mapped_items = Map.get(input, "mapped_items", []) |> List.wrap()
        highlights = mapped_items |> Enum.map(&extract_highlight/1) |> Enum.reject(&is_nil/1)

        %{
          "summary" =>
            "Reduced #{length(mapped_items)} items for query #{Map.get(input, "query", "unknown")}",
          "highlights" => Enum.take(highlights, 4),
          "item_count" => length(mapped_items),
          "confidence" => 0.64
        }

      _ ->
        %{"result" => stringify_keys(input)}
    end
  end

  defp first_tool_name(input) do
    input
    |> Map.get("tools", [])
    |> List.first()
    |> case do
      %{"name" => name} when is_binary(name) -> name
      %{name: name} when is_binary(name) -> name
      _ -> "none"
    end
  end

  defp extract_message_lines(messages) when is_list(messages) do
    messages
    |> Enum.map(fn
      %{"content" => content} when is_binary(content) -> content
      %{content: content} when is_binary(content) -> content
      _ -> nil
    end)
    |> Enum.reject(&is_nil/1)
  end

  defp extract_message_lines(_messages), do: []

  defp item_digest(item) when is_map(item) do
    item
    |> Enum.sort_by(fn {key, _value} -> to_string(key) end)
    |> Enum.take(2)
    |> Enum.map_join(", ", fn {key, value} -> "#{key}=#{inspect(value, limit: 4)}" end)
  end

  defp item_digest(_item), do: "no details"

  defp extract_highlight(%{"summary" => summary}) when is_binary(summary), do: summary
  defp extract_highlight(%{summary: summary}) when is_binary(summary), do: summary
  defp extract_highlight(_), do: nil

  defp normalize_number(value) when is_integer(value), do: value
  defp normalize_number(value) when is_float(value), do: value

  defp normalize_number(value) when is_binary(value) do
    case Float.parse(String.trim(value)) do
      {parsed, _rest} -> parsed
      :error -> 0
    end
  end

  defp normalize_number(_), do: 0

  defp stringify_keys(%{} = map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), stringify_keys(value)}
      {key, value} -> {to_string(key), stringify_keys(value)}
    end)
  end

  defp stringify_keys(list) when is_list(list), do: Enum.map(list, &stringify_keys/1)
  defp stringify_keys(value), do: value
end
