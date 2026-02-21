defmodule OpenAgentsRuntime.Hooks.Runner do
  @moduledoc """
  Deterministic runtime hook registry + execution runner.

  Supports OpenClaw-aligned lifecycle hook phases:
  - `before_model_resolve`
  - `before_prompt_build`
  - `before_tool_call`
  - `after_tool_call`
  - `before_message_persist`
  """

  @hook_names MapSet.new([
                :before_model_resolve,
                :before_prompt_build,
                :before_tool_call,
                :after_tool_call,
                :before_message_persist
              ])

  @type hook_name ::
          :before_model_resolve
          | :before_prompt_build
          | :before_tool_call
          | :after_tool_call
          | :before_message_persist

  @type hook_registration :: %{
          id: String.t(),
          hook: hook_name(),
          priority: integer(),
          handler: (map(), map() -> map() | nil | :ok)
        }

  @type hook_event :: {String.t(), map()}

  @type modifying_result :: %{
          result: map() | nil,
          events: [hook_event()]
        }

  @spec normalize_registry([map()]) :: [hook_registration()]
  def normalize_registry(registrations) when is_list(registrations) do
    registrations
    |> Enum.flat_map(fn registration ->
      case normalize_registration(registration) do
        {:ok, normalized} -> [normalized]
        :error -> []
      end
    end)
    |> Enum.sort_by(fn hook -> {-hook.priority, hook.id} end)
  end

  @spec run_modifying_hook(
          [hook_registration()],
          hook_name(),
          map(),
          map(),
          (map(), map() -> map())
        ) :: modifying_result()
  def run_modifying_hook(registry, hook_name, event, context, merge_fun)
      when is_list(registry) and is_map(event) and is_map(context) and is_function(merge_fun, 2) do
    hooks = hooks_for(registry, hook_name)

    Enum.reduce(hooks, %{result: nil, events: []}, fn hook, acc ->
      case invoke_hook(hook, event, context) do
        {:ok, nil} ->
          acc

        {:ok, :ok} ->
          acc

        {:ok, result} when is_map(result) ->
          merged =
            if is_map(acc.result) do
              merge_fun.(acc.result, stringify_keys(result))
            else
              stringify_keys(result)
            end

          %{acc | result: merged, events: acc.events ++ [hook_applied_event(hook, result)]}

        {:ok, _other} ->
          %{acc | events: acc.events ++ [hook_error_event(hook, "invalid_hook_result")]}

        {:error, reason} ->
          %{acc | events: acc.events ++ [hook_error_event(hook, reason)]}
      end
    end)
  end

  @spec run_void_hook([hook_registration()], hook_name(), map(), map()) :: [hook_event()]
  def run_void_hook(registry, hook_name, event, context)
      when is_list(registry) and is_map(event) and is_map(context) do
    hooks = hooks_for(registry, hook_name)

    Enum.flat_map(hooks, fn hook ->
      case invoke_hook(hook, event, context) do
        {:ok, _result} -> [hook_applied_event(hook, %{})]
        {:error, reason} -> [hook_error_event(hook, reason)]
      end
    end)
  end

  @spec merge_before_model_resolve(map(), map()) :: map()
  def merge_before_model_resolve(acc, next) when is_map(acc) and is_map(next) do
    %{}
    |> maybe_put("model_override", acc["model_override"] || next["model_override"])
    |> maybe_put("provider_override", acc["provider_override"] || next["provider_override"])
  end

  @spec merge_before_prompt_build(map(), map()) :: map()
  def merge_before_prompt_build(acc, next) when is_map(acc) and is_map(next) do
    prepend_context =
      case {acc["prepend_context"], next["prepend_context"]} do
        {left, right} when is_binary(left) and is_binary(right) and left != "" and right != "" ->
          "#{left}\n\n#{right}"

        {_left, right} when is_binary(right) and right != "" ->
          right

        {left, _right} ->
          left
      end

    %{}
    |> maybe_put("system_prompt", acc["system_prompt"] || next["system_prompt"])
    |> maybe_put("prepend_context", prepend_context)
  end

  @spec merge_before_message_persist(map(), map()) :: map()
  def merge_before_message_persist(acc, next) when is_map(acc) and is_map(next) do
    payload =
      [acc["payload"], next["payload"]]
      |> Enum.filter(&is_map/1)
      |> Enum.reduce(%{}, fn map, merged -> Map.merge(map, merged) end)

    %{}
    |> maybe_put("event_type", acc["event_type"] || next["event_type"])
    |> maybe_put("payload", if(map_size(payload) == 0, do: nil, else: payload))
  end

  defp normalize_registration(registration) when is_map(registration) do
    id =
      registration
      |> map_get([:id, "id"])
      |> to_string_safe()
      |> String.trim()

    hook =
      registration
      |> map_get([:hook, "hook", :hook_name, "hook_name"])
      |> normalize_hook_name()

    priority =
      registration
      |> map_get([:priority, "priority"])
      |> normalize_int(0)

    handler =
      registration
      |> map_get([:handler, "handler"])

    cond do
      id == "" -> :error
      is_nil(hook) -> :error
      not is_function(handler, 2) -> :error
      true -> {:ok, %{id: id, hook: hook, priority: priority, handler: handler}}
    end
  end

  defp normalize_registration(_), do: :error

  defp normalize_hook_name(hook_name) when is_atom(hook_name) do
    if MapSet.member?(@hook_names, hook_name), do: hook_name, else: nil
  end

  defp normalize_hook_name(hook_name) when is_binary(hook_name) do
    case String.trim(hook_name) do
      "" ->
        nil

      value ->
        normalized =
          value
          |> String.replace("-", "_")
          |> String.downcase()

        try do
          atom = String.to_existing_atom(normalized)
          if MapSet.member?(@hook_names, atom), do: atom, else: nil
        rescue
          ArgumentError -> nil
        end
    end
  end

  defp normalize_hook_name(_), do: nil

  defp hooks_for(registry, hook_name), do: Enum.filter(registry, &(&1.hook == hook_name))

  defp invoke_hook(hook, event, context) do
    try do
      {:ok, hook.handler.(event, context)}
    rescue
      error ->
        {:error, Exception.message(error)}
    catch
      kind, reason ->
        {:error, "#{kind}:#{inspect(reason)}"}
    end
  end

  defp hook_applied_event(hook, result) do
    {"run.hook_applied",
     %{
       "hook_id" => hook.id,
       "hook_name" => Atom.to_string(hook.hook),
       "priority" => hook.priority,
       "result_keys" => result_keys(result)
     }}
  end

  defp hook_error_event(hook, reason) do
    {"run.hook_error",
     %{
       "hook_id" => hook.id,
       "hook_name" => Atom.to_string(hook.hook),
       "priority" => hook.priority,
       "reason" => to_string_safe(reason)
     }}
  end

  defp result_keys(result) when is_map(result),
    do: result |> Map.keys() |> Enum.map(&to_string_safe/1)

  defp result_keys(_), do: []

  defp to_string_safe(value) when is_binary(value), do: value
  defp to_string_safe(value) when is_atom(value), do: Atom.to_string(value)
  defp to_string_safe(value), do: to_string(value)

  defp normalize_int(value, _fallback) when is_integer(value), do: value

  defp normalize_int(value, fallback) when is_binary(value) do
    case Integer.parse(String.trim(value)) do
      {parsed, _rest} -> parsed
      :error -> fallback
    end
  end

  defp normalize_int(_value, fallback), do: fallback

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp map_get(map, keys) when is_map(map) and is_list(keys) do
    Enum.find_value(keys, fn key ->
      if Map.has_key?(map, key), do: Map.get(map, key), else: nil
    end)
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)
end
