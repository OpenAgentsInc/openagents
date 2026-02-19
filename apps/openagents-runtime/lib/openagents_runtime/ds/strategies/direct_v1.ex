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

  defp stringify_keys(%{} = map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), stringify_keys(value)}
      {key, value} -> {to_string(key), stringify_keys(value)}
    end)
  end

  defp stringify_keys(list) when is_list(list), do: Enum.map(list, &stringify_keys/1)
  defp stringify_keys(value), do: value
end
