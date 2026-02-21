defmodule OpenAgentsRuntime.Sync.Replay do
  @moduledoc """
  DB-backed replay reads for Khala topic/watermark catch-up.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.StreamEvent

  @default_batch_size 200

  @type replay_batch :: %{
          events: [StreamEvent.t()],
          next_watermark: non_neg_integer(),
          head_watermark: non_neg_integer()
        }

  @spec fetch_batch(String.t(), non_neg_integer(), keyword()) ::
          {:ok, replay_batch()} | {:error, term()}
  def fetch_batch(topic, watermark, opts \\ [])
      when is_binary(topic) and is_integer(watermark) and watermark >= 0 do
    batch_size = batch_size(opts)

    events =
      from(stream_event in StreamEvent,
        where: stream_event.topic == ^topic and stream_event.watermark > ^watermark,
        order_by: [asc: stream_event.watermark],
        limit: ^batch_size
      )
      |> Repo.all()

    head_watermark = head_watermark(topic)

    next_watermark =
      case List.last(events) do
        %StreamEvent{watermark: value} -> value
        nil -> watermark
      end

    {:ok,
     %{
       events: events,
       next_watermark: next_watermark,
       head_watermark: head_watermark
     }}
  end

  @spec head_watermark(String.t()) :: non_neg_integer()
  def head_watermark(topic) when is_binary(topic) do
    from(stream_event in StreamEvent,
      where: stream_event.topic == ^topic,
      select: max(stream_event.watermark)
    )
    |> Repo.one()
    |> case do
      nil -> 0
      value -> value
    end
  end

  @spec oldest_watermark(String.t()) :: non_neg_integer() | nil
  def oldest_watermark(topic) when is_binary(topic) do
    from(stream_event in StreamEvent,
      where: stream_event.topic == ^topic,
      select: min(stream_event.watermark)
    )
    |> Repo.one()
  end

  defp batch_size(opts) do
    opts_batch_size = Keyword.get(opts, :batch_size)

    configured_batch_size =
      Application.get_env(:openagents_runtime, :khala_sync_replay_batch_size, @default_batch_size)

    value = opts_batch_size || configured_batch_size

    if is_integer(value) and value > 0, do: value, else: @default_batch_size
  end
end
