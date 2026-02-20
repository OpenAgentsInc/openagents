defmodule OpenAgentsRuntime.Sync.Notifier do
  @moduledoc """
  PubSub notifications for Khala stream updates.
  """

  @spec subscribe(String.t()) :: :ok | {:error, term()}
  def subscribe(topic) when is_binary(topic) do
    Phoenix.PubSub.subscribe(OpenAgentsRuntime.PubSub, pubsub_topic(topic))
  end

  @spec unsubscribe(String.t()) :: :ok
  def unsubscribe(topic) when is_binary(topic) do
    Phoenix.PubSub.unsubscribe(OpenAgentsRuntime.PubSub, pubsub_topic(topic))
  end

  @spec broadcast_stream_event(String.t(), non_neg_integer()) :: :ok
  def broadcast_stream_event(topic, watermark)
      when is_binary(topic) and is_integer(watermark) and watermark >= 0 do
    Phoenix.PubSub.broadcast(
      OpenAgentsRuntime.PubSub,
      pubsub_topic(topic),
      {:sync_stream_event, topic, watermark}
    )
  end

  @spec pubsub_topic(String.t()) :: String.t()
  def pubsub_topic(topic) when is_binary(topic), do: "sync_stream:" <> topic
end
