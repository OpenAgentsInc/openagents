defmodule OpenAgentsRuntime.Sync.PayloadHash do
  @moduledoc """
  Deterministic payload hashing helpers for Khala sync payloads.
  """

  @type json_value ::
          nil
          | boolean()
          | number()
          | String.t()
          | [json_value()]
          | %{optional(any()) => json_value()}

  @spec canonical_json(json_value()) :: String.t()
  def canonical_json(value), do: encode_canonical(value)

  @spec sha256_canonical_json(json_value()) :: String.t()
  def sha256_canonical_json(value) do
    value
    |> canonical_json()
    |> sha256_bytes()
  end

  @spec sha256_bytes(binary()) :: String.t()
  def sha256_bytes(payload) when is_binary(payload) do
    "sha256:" <>
      (payload
       |> then(&:crypto.hash(:sha256, &1))
       |> Base.encode16(case: :lower))
  end

  defp encode_canonical(%{} = map) do
    entries =
      map
      |> Enum.map(fn {key, value} -> {to_string(key), value} end)
      |> Enum.sort_by(fn {key, _value} -> key end)

    "{" <>
      Enum.map_join(entries, ",", fn {key, value} ->
        Jason.encode!(key) <> ":" <> encode_canonical(value)
      end) <> "}"
  end

  defp encode_canonical(list) when is_list(list) do
    "[" <> Enum.map_join(list, ",", &encode_canonical/1) <> "]"
  end

  defp encode_canonical(value) when is_binary(value), do: Jason.encode!(value)
  defp encode_canonical(value) when is_boolean(value), do: Jason.encode!(value)
  defp encode_canonical(nil), do: "null"
  defp encode_canonical(value) when is_integer(value), do: Integer.to_string(value)
  defp encode_canonical(value) when is_float(value), do: Jason.encode!(value)

  defp encode_canonical(other) do
    raise ArgumentError, "unsupported value for canonical JSON encoding: #{inspect(other)}"
  end
end
