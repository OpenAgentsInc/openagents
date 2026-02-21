defmodule OpenAgentsRuntime.Sync.KhalaFrame do
  @moduledoc """
  Canonical Khala frame envelope builder for WS transport.
  """

  @schema_version 1

  @kind_map %{
    subscribed: "KHALA_FRAME_KIND_SUBSCRIBED",
    update_batch: "KHALA_FRAME_KIND_UPDATE_BATCH",
    heartbeat: "KHALA_FRAME_KIND_HEARTBEAT",
    error: "KHALA_FRAME_KIND_ERROR"
  }

  @type kind :: :subscribed | :update_batch | :heartbeat | :error

  @spec schema_version() :: pos_integer()
  def schema_version, do: @schema_version

  @spec build(binary(), non_neg_integer(), kind(), map()) ::
          {:ok, map()}
          | {:error, :invalid_topic | :invalid_seq | :invalid_kind | :invalid_payload}
  def build(topic, seq, kind, payload)
      when is_binary(topic) and is_integer(seq) and is_map(payload) do
    trimmed_topic = String.trim(topic)
    kind_string = Map.get(@kind_map, kind)

    cond do
      trimmed_topic == "" ->
        {:error, :invalid_topic}

      seq < 0 ->
        {:error, :invalid_seq}

      not is_binary(kind_string) ->
        {:error, :invalid_kind}

      true ->
        with {:ok, encoded_payload} <- Jason.encode(payload) do
          {:ok,
           %{
             "topic" => topic,
             "seq" => seq,
             "kind" => kind_string,
             "payload_bytes" => Base.encode64(encoded_payload),
             "schema_version" => @schema_version
           }}
        else
          {:error, _reason} -> {:error, :invalid_payload}
        end
    end
  end

  def build(_topic, _seq, _kind, _payload), do: {:error, :invalid_payload}

  @spec decode_payload(map()) :: {:ok, map()} | {:error, :invalid_payload}
  def decode_payload(%{"payload_bytes" => encoded_payload}) when is_binary(encoded_payload) do
    with {:ok, decoded_bytes} <- Base.decode64(encoded_payload),
         {:ok, decoded_payload} <- Jason.decode(decoded_bytes) do
      {:ok, decoded_payload}
    else
      _error -> {:error, :invalid_payload}
    end
  end

  def decode_payload(_frame), do: {:error, :invalid_payload}
end
