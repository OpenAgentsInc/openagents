defmodule OpenAgentsRuntime.Runs.EventListener do
  @moduledoc """
  LISTEN/NOTIFY bridge that broadcasts run wakeups over PubSub.
  """

  use GenServer

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.EventNotifier

  @all_events_topic "runtime:run_events"

  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @spec subscribe_all() :: :ok | {:error, term()}
  def subscribe_all do
    Phoenix.PubSub.subscribe(OpenAgentsRuntime.PubSub, @all_events_topic)
  end

  @spec subscribe(String.t()) :: :ok | {:error, term()}
  def subscribe(run_id) when is_binary(run_id) do
    Phoenix.PubSub.subscribe(OpenAgentsRuntime.PubSub, run_topic(run_id))
  end

  @spec run_topic(String.t()) :: String.t()
  def run_topic(run_id), do: "runtime:run_events:" <> run_id

  @impl true
  def init(_opts) do
    config =
      Repo.config()
      |> Keyword.take([:hostname, :port, :username, :password, :database, :ssl, :socket_dir])

    {:ok, notifications_pid} = Postgrex.Notifications.start_link(config)
    {:ok, listen_ref} = Postgrex.Notifications.listen(notifications_pid, EventNotifier.channel())

    {:ok, %{notifications_pid: notifications_pid, listen_ref: listen_ref}}
  end

  @impl true
  def handle_info({:notification, _pid, _ref, _channel, payload}, state) do
    case EventNotifier.decode_payload(payload) do
      {:ok, %{run_id: run_id, seq: seq} = decoded} ->
        message = {:run_event_notification, decoded}

        Phoenix.PubSub.broadcast(OpenAgentsRuntime.PubSub, @all_events_topic, message)
        Phoenix.PubSub.broadcast(OpenAgentsRuntime.PubSub, run_topic(run_id), message)

        :telemetry.execute(
          [:openagents_runtime, :run_events, :notify],
          %{count: 1},
          %{run_id: run_id, seq: seq}
        )

      {:error, :invalid} ->
        :ok
    end

    {:noreply, state}
  end
end
