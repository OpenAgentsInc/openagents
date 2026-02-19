defmodule OpenAgentsRuntime.Security.Sanitizer do
  @moduledoc """
  Centralized sanitization for secrets and PII across runtime boundaries.
  """

  @redacted "[REDACTED]"
  @redacted_email "[REDACTED_EMAIL]"
  @redacted_phone "[REDACTED_PHONE]"

  @sensitive_exact_keys MapSet.new([
                          "authorization",
                          "proxy_authorization",
                          "cookie",
                          "set_cookie",
                          "password",
                          "passphrase",
                          "secret",
                          "api_key",
                          "apikey",
                          "x_api_key",
                          "access_token",
                          "refresh_token",
                          "token",
                          "client_secret",
                          "private_key"
                        ])

  @pii_exact_keys MapSet.new([
                    "email",
                    "e_mail",
                    "phone",
                    "phone_number",
                    "ssn",
                    "address",
                    "street",
                    "postal_code",
                    "zip"
                  ])

  @key_suffixes ["_token", "_secret", "_password", "_api_key", "_apikey"]
  @preserve_redaction_key "__preserve__"

  @type option :: {:preserve_keys, [atom() | String.t()]}

  @spec sanitize(term(), [option()]) :: term()
  def sanitize(value, opts \\ []) do
    preserve_keys =
      opts
      |> Keyword.get(:preserve_keys, [])
      |> Enum.map(&normalize_key/1)
      |> MapSet.new()

    sanitize_value(value, preserve_keys)
  end

  defp sanitize_value(%{} = map, preserve_keys) do
    map
    |> Enum.map(fn {key, value} ->
      normalized_key = normalize_key(key)

      sanitized_value =
        cond do
          MapSet.member?(preserve_keys, normalized_key) ->
            sanitize_value(value, preserve_keys)

          sensitive_key?(normalized_key) ->
            @redacted

          pii_key?(normalized_key) ->
            pii_redaction(normalized_key)

          true ->
            sanitize_value(value, preserve_keys)
        end

      {key, sanitized_value}
    end)
    |> Enum.into(%{})
  end

  defp sanitize_value(list, preserve_keys) when is_list(list) do
    Enum.map(list, &sanitize_value(&1, preserve_keys))
  end

  defp sanitize_value(value, _preserve_keys) when is_binary(value), do: sanitize_binary(value)
  defp sanitize_value(value, _preserve_keys), do: value

  defp sanitize_binary(value) do
    value
    |> String.replace(~r/Bearer\s+[A-Za-z0-9\-\._~\+\/=]+/i, "Bearer #{@redacted}")
    |> String.replace(~r/sk-[A-Za-z0-9_\-]+/, @redacted)
    |> String.replace(
      ~r/\b[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b/,
      @redacted
    )
    |> String.replace(~r/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/i, @redacted_email)
    |> String.replace(~r/\+?\d[\d\-\s\(\)]{7,}\d/, @redacted_phone)
  end

  defp sensitive_key?(normalized_key) do
    MapSet.member?(@sensitive_exact_keys, normalized_key) or
      Enum.any?(@key_suffixes, &String.ends_with?(normalized_key, &1))
  end

  defp pii_key?(normalized_key), do: MapSet.member?(@pii_exact_keys, normalized_key)

  defp pii_redaction("email"), do: @redacted_email
  defp pii_redaction("e_mail"), do: @redacted_email
  defp pii_redaction("phone"), do: @redacted_phone
  defp pii_redaction("phone_number"), do: @redacted_phone
  defp pii_redaction(_), do: @redacted

  defp normalize_key(@preserve_redaction_key), do: @preserve_redaction_key
  defp normalize_key(key) when is_atom(key), do: key |> Atom.to_string() |> normalize_key()

  defp normalize_key(key) when is_binary(key) do
    key
    |> String.downcase()
    |> String.replace("-", "_")
    |> String.trim()
  end

  defp normalize_key(key), do: key |> to_string() |> normalize_key()
end
