defmodule OpenAgentsRuntime.Integrations.LaravelEventMapper do
  @moduledoc """
  Maps runtime-native events to the Laravel AI SDK SSE protocol.

  Output frames are emitted as SSE lines with:
  - `event` (typically `message`)
  - `id` (SSE cursor id)
  - `data` (JSON payload or `[DONE]`)
  """

  @type sse_frame :: %{event: String.t(), id: String.t() | nil, data: String.t()}

  @spec map_runtime_event(String.t(), non_neg_integer(), String.t(), map()) :: [sse_frame()]
  def map_runtime_event(run_id, seq, event_type, payload)
      when is_binary(run_id) and is_integer(seq) and is_binary(event_type) and is_map(payload) do
    payload = stringify_keys(payload)
    seq_id = Integer.to_string(seq)

    case event_type do
      "run.started" ->
        [json_frame(seq_id, %{"type" => "start", "runId" => run_id, "meta" => payload})]

      "run.delta" ->
        [
          json_frame(seq_id, %{
            "type" => "text-delta",
            "runId" => run_id,
            "delta" => delta(payload)
          })
        ]

      "text.delta" ->
        [
          json_frame(seq_id, %{
            "type" => "text-delta",
            "runId" => run_id,
            "delta" => delta(payload)
          })
        ]

      "tool.call" ->
        [json_frame(seq_id, %{"type" => "tool-call", "runId" => run_id, "toolCall" => payload})]

      "tool.result" ->
        [
          json_frame(seq_id, %{
            "type" => "tool-result",
            "runId" => run_id,
            "toolResult" => payload
          })
        ]

      "run.finished" ->
        [
          json_frame(seq_id, %{"type" => "finish", "runId" => run_id, "meta" => payload}),
          done_frame(seq_id)
        ]

      _ ->
        [
          json_frame(seq_id, %{
            "type" => "event",
            "runId" => run_id,
            "eventType" => event_type,
            "payload" => payload
          })
        ]
    end
  end

  @spec to_sse_chunk(sse_frame()) :: String.t()
  def to_sse_chunk(%{event: event, id: id, data: data})
      when is_binary(event) and is_binary(data) do
    id_line = if is_binary(id), do: "id: #{id}\n", else: ""
    "event: #{event}\n#{id_line}data: #{data}\n\n"
  end

  defp json_frame(id, payload) do
    %{event: "message", id: id, data: Jason.encode!(payload)}
  end

  defp done_frame(id) do
    %{event: "message", id: id, data: "[DONE]"}
  end

  defp delta(payload) do
    payload["delta"] || payload["text"] || ""
  end

  defp stringify_keys(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end
end
