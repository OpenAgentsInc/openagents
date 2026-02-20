defmodule OpenAgentsRuntimeWeb.SyncChannel do
  @moduledoc """
  Khala sync channel for authenticated topic subscription intent.

  Replay/live delivery is implemented in subsequent stages.
  """

  use OpenAgentsRuntimeWeb, :channel

  @known_topics MapSet.new([
                  "runtime.run_summaries",
                  "runtime.codex_worker_summaries",
                  "runtime.notifications"
                ])

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
     |> assign(:subscribed_topics, MapSet.new())}
  end

  def join(_topic, _params, _socket), do: {:error, %{"code" => "bad_topic"}}

  @impl true
  def handle_in("sync:subscribe", %{"topics" => topics}, socket) do
    with {:ok, normalized_topics} <- normalize_topics(topics),
         :ok <- ensure_known_topics(normalized_topics),
         :ok <- ensure_allowed_topics(normalized_topics, socket.assigns[:allowed_topics] || []) do
      subscribed_topics = MapSet.new(normalized_topics)

      {:reply,
       {:ok,
        %{
          "subscription_id" => socket.assigns[:subscription_id],
          "topics" => normalized_topics,
          "current_watermarks" => []
        }}, assign(socket, :subscribed_topics, subscribed_topics)}
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
    end
  end

  def handle_in("sync:subscribe", _payload, socket) do
    {:reply, {:error, %{"code" => "bad_subscription"}}, socket}
  end

  @impl true
  def handle_in("sync:unsubscribe", %{"topics" => topics}, socket) do
    with {:ok, normalized_topics} <- normalize_topics(topics) do
      current = socket.assigns[:subscribed_topics] || MapSet.new()
      next = Enum.reduce(normalized_topics, current, &MapSet.delete(&2, &1))

      {:reply,
       {:ok,
        %{
          "subscription_id" => socket.assigns[:subscription_id],
          "topics" => MapSet.to_list(next)
        }}, assign(socket, :subscribed_topics, next)}
    else
      {:error, :bad_subscription} ->
        {:reply, {:error, %{"code" => "bad_subscription"}}, socket}
    end
  end

  def handle_in("sync:unsubscribe", _payload, socket) do
    {:reply, {:error, %{"code" => "bad_subscription"}}, socket}
  end

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

  defp ensure_known_topics(topics) do
    unknown = Enum.reject(topics, &MapSet.member?(@known_topics, &1))

    if unknown == [], do: :ok, else: {:error, {:forbidden_topic, unknown}}
  end

  defp ensure_allowed_topics(requested_topics, allowed_topics) do
    forbidden = requested_topics -- allowed_topics

    if forbidden == [], do: :ok, else: {:error, {:forbidden_topic, forbidden}}
  end

  defp subscription_id(socket) do
    "sub_#{socket.id || System.unique_integer([:positive])}"
  end
end
