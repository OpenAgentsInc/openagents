defmodule OpenAgentsRuntimeWeb.SyncChannel do
  @moduledoc """
  Khala sync channel for authenticated topic subscription + replay-on-subscribe.
  """

  use OpenAgentsRuntimeWeb, :channel

  alias OpenAgentsRuntime.Sync.Notifier
  alias OpenAgentsRuntime.Sync.Replay
  alias OpenAgentsRuntime.Sync.StreamEvent

  @known_topics MapSet.new([
                  "runtime.run_summaries",
                  "runtime.codex_worker_summaries",
                  "runtime.notifications"
                ])

  @default_replay_batch_size 200

  @impl true
  def join("sync:v1", _params, socket) do
    allowed_topics = socket.assigns[:allowed_topics] || []

    response = %{
      "subscription_id" => subscription_id(socket),
      "allowed_topics" => allowed_topics,
      "current_watermarks" => []
    }

    {:ok, response,
     socket
     |> assign(:subscription_id, response["subscription_id"])
     |> assign(:subscribed_topics, MapSet.new())
     |> assign(:topic_watermarks, %{})}
  end

  def join(_topic, _params, _socket), do: {:error, %{"code" => "bad_topic"}}

  @impl true
  def handle_in("sync:subscribe", payload, socket) when is_map(payload) do
    with {:ok, normalized_topics} <- normalize_topics(Map.get(payload, "topics")),
         :ok <- ensure_known_topics(normalized_topics),
         :ok <- ensure_allowed_topics(normalized_topics, socket.assigns[:allowed_topics] || []),
         {:ok, resume_after} <- parse_resume_after(Map.get(payload, "resume_after")),
         :ok <- ensure_fresh_resume(normalized_topics, resume_after) do
      replay_batch_size = replay_batch_size(payload)

      socket = subscribe_new_topics(socket, normalized_topics)

      {socket, topic_watermarks} =
        replay_topics(socket, normalized_topics, resume_after, replay_batch_size)

      current_topics = socket.assigns[:subscribed_topics] || MapSet.new()
      subscribed_topics = MapSet.union(current_topics, MapSet.new(normalized_topics))

      response = %{
        "subscription_id" => socket.assigns[:subscription_id],
        "topics" => normalized_topics,
        "current_watermarks" => current_watermarks(normalized_topics, topic_watermarks)
      }

      {:reply, {:ok, response},
       socket
       |> assign(:subscribed_topics, subscribed_topics)
       |> assign(:topic_watermarks, topic_watermarks)}
    else
      {:error, :bad_subscription} ->
        {:reply, {:error, %{"code" => "bad_subscription"}}, socket}

      {:error, {:forbidden_topic, forbidden_topics}} ->
        {:reply,
         {:error,
          %{
            "code" => "forbidden_topic",
            "forbidden_topics" => forbidden_topics
          }}, socket}

      {:error, {:stale_cursor, stale_topics}} ->
        error_payload = stale_cursor_payload(stale_topics)
        push(socket, "sync:error", error_payload)
        {:reply, {:error, error_payload}, socket}
    end
  end

  def handle_in("sync:subscribe", _payload, socket) do
    {:reply, {:error, %{"code" => "bad_subscription"}}, socket}
  end

  @impl true
  def handle_in("sync:unsubscribe", %{"topics" => topics}, socket) do
    with {:ok, normalized_topics} <- normalize_topics(topics) do
      current_topics = socket.assigns[:subscribed_topics] || MapSet.new()

      {next_topics, next_watermarks} =
        Enum.reduce(
          normalized_topics,
          {current_topics, socket.assigns[:topic_watermarks] || %{}},
          fn topic, {topics_acc, watermarks_acc} ->
            Notifier.unsubscribe(topic)

            {
              MapSet.delete(topics_acc, topic),
              Map.delete(watermarks_acc, topic)
            }
          end
        )

      {:reply,
       {:ok,
        %{
          "subscription_id" => socket.assigns[:subscription_id],
          "topics" => MapSet.to_list(next_topics)
        }},
       socket
       |> assign(:subscribed_topics, next_topics)
       |> assign(:topic_watermarks, next_watermarks)}
    else
      {:error, :bad_subscription} ->
        {:reply, {:error, %{"code" => "bad_subscription"}}, socket}
    end
  end

  def handle_in("sync:unsubscribe", _payload, socket) do
    {:reply, {:error, %{"code" => "bad_subscription"}}, socket}
  end

  @impl true
  def handle_info({:sync_stream_event, topic, _watermark}, socket) do
    subscribed_topics = socket.assigns[:subscribed_topics] || MapSet.new()

    if MapSet.member?(subscribed_topics, topic) do
      {socket, topic_watermarks} = replay_topics(socket, [topic], %{}, replay_batch_size(%{}))
      {:noreply, assign(socket, :topic_watermarks, topic_watermarks)}
    else
      {:noreply, socket}
    end
  end

  def handle_info(_message, socket), do: {:noreply, socket}

  defp replay_topics(socket, topics, resume_after, replay_batch_size) do
    topic_watermarks = socket.assigns[:topic_watermarks] || %{}

    Enum.reduce(topics, {socket, topic_watermarks}, fn topic, {socket_acc, watermarks_acc} ->
      start_watermark =
        Map.get(resume_after, topic, Map.get(watermarks_acc, topic, 0))

      {socket_acc, final_watermark} =
        replay_topic(socket_acc, topic, start_watermark, replay_batch_size)

      {socket_acc, Map.put(watermarks_acc, topic, final_watermark)}
    end)
  end

  defp replay_topic(socket, topic, current_watermark, replay_batch_size) do
    {:ok, %{events: events, next_watermark: next_watermark, head_watermark: head_watermark}} =
      Replay.fetch_batch(topic, current_watermark, batch_size: replay_batch_size)

    replay_complete = next_watermark >= head_watermark

    if events != [] do
      push(socket, "sync:update_batch", %{
        "updates" => Enum.map(events, &stream_event_to_update/1),
        "replay_complete" => replay_complete,
        "head_watermarks" => [
          %{
            "topic" => topic,
            "watermark" => head_watermark
          }
        ]
      })
    end

    if replay_complete do
      {socket, max(next_watermark, current_watermark)}
    else
      replay_topic(socket, topic, next_watermark, replay_batch_size)
    end
  end

  defp stream_event_to_update(%StreamEvent{} = stream_event) do
    %{
      "topic" => stream_event.topic,
      "doc_key" => stream_event.doc_key,
      "doc_version" => stream_event.doc_version,
      "payload" => stream_event.payload,
      "payload_hash" => maybe_base64(stream_event.payload_hash),
      "watermark" => stream_event.watermark,
      "hydration_required" => is_nil(stream_event.payload)
    }
  end

  defp maybe_base64(nil), do: nil
  defp maybe_base64(value) when is_binary(value), do: Base.encode64(value)

  defp normalize_topics(topics) when is_list(topics) do
    normalized =
      topics
      |> Enum.filter(&is_binary/1)
      |> Enum.map(&String.trim/1)
      |> Enum.reject(&(&1 == ""))
      |> Enum.uniq()

    if normalized == [], do: {:error, :bad_subscription}, else: {:ok, normalized}
  end

  defp normalize_topics(_topics), do: {:error, :bad_subscription}

  defp parse_resume_after(nil), do: {:ok, %{}}

  defp parse_resume_after(resume_after) when is_map(resume_after) do
    parsed =
      Enum.reduce_while(resume_after, %{}, fn
        {topic, watermark}, acc
        when is_binary(topic) and is_integer(watermark) and watermark >= 0 ->
          {:cont, Map.put(acc, topic, watermark)}

        _entry, _acc ->
          {:halt, :error}
      end)

    case parsed do
      :error -> {:error, :bad_subscription}
      map -> {:ok, map}
    end
  end

  defp parse_resume_after(resume_after) when is_list(resume_after) do
    parsed =
      Enum.reduce_while(resume_after, %{}, fn
        %{"topic" => topic, "watermark" => watermark}, acc
        when is_binary(topic) and is_integer(watermark) and watermark >= 0 ->
          {:cont, Map.put(acc, topic, watermark)}

        _entry, _acc ->
          {:halt, :error}
      end)

    case parsed do
      :error -> {:error, :bad_subscription}
      map -> {:ok, map}
    end
  end

  defp parse_resume_after(_resume_after), do: {:error, :bad_subscription}

  defp ensure_known_topics(topics) do
    unknown = Enum.reject(topics, &MapSet.member?(@known_topics, &1))

    if unknown == [], do: :ok, else: {:error, {:forbidden_topic, unknown}}
  end

  defp ensure_allowed_topics(requested_topics, allowed_topics) do
    forbidden = requested_topics -- allowed_topics

    if forbidden == [], do: :ok, else: {:error, {:forbidden_topic, forbidden}}
  end

  defp ensure_fresh_resume(topics, resume_after) when is_map(resume_after) do
    stale_topics =
      topics
      |> Enum.filter(&Map.has_key?(resume_after, &1))
      |> Enum.reduce([], fn topic, acc ->
        resume_watermark = Map.fetch!(resume_after, topic)
        oldest_watermark = Replay.oldest_watermark(topic)
        retention_floor = retention_floor(oldest_watermark)

        if resume_watermark < retention_floor do
          [
            %{
              "topic" => topic,
              "resume_after" => resume_watermark,
              "retention_floor" => retention_floor
            }
            | acc
          ]
        else
          acc
        end
      end)
      |> Enum.reverse()

    if stale_topics == [], do: :ok, else: {:error, {:stale_cursor, stale_topics}}
  end

  defp subscribe_new_topics(socket, topics) do
    current = socket.assigns[:subscribed_topics] || MapSet.new()

    Enum.each(topics, fn topic ->
      if not MapSet.member?(current, topic) do
        :ok = Notifier.subscribe(topic)
      end
    end)

    socket
  end

  defp replay_batch_size(payload) when is_map(payload) do
    value =
      payload
      |> Map.get("replay_batch_size")
      |> case do
        number when is_integer(number) and number > 0 ->
          number

        _other ->
          Application.get_env(
            :openagents_runtime,
            :khala_sync_replay_batch_size,
            @default_replay_batch_size
          )
      end

    if is_integer(value) and value > 0, do: value, else: @default_replay_batch_size
  end

  defp current_watermarks(topics, topic_watermarks) do
    Enum.map(topics, fn topic ->
      %{
        "topic" => topic,
        "watermark" => Map.get(topic_watermarks, topic, 0)
      }
    end)
  end

  defp subscription_id(socket) do
    "sub_#{socket.id || System.unique_integer([:positive])}"
  end

  defp retention_floor(nil), do: 0

  defp retention_floor(oldest_watermark) when is_integer(oldest_watermark),
    do: max(oldest_watermark - 1, 0)

  defp stale_cursor_payload(stale_topics) do
    %{
      "code" => "stale_cursor",
      "message" => "cursor is older than retention floor",
      "full_resync_required" => true,
      "stale_topics" => stale_topics
    }
  end
end
