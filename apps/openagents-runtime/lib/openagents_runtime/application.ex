defmodule OpenAgentsRuntime.Application do
  @moduledoc """
  Application entrypoint for the OpenAgents runtime service.
  """

  use Application

  @impl true
  def start(_type, _args) do
    children = [
      OpenAgentsRuntimeWeb.Telemetry,
      OpenAgentsRuntime.Repo,
      {DNSCluster,
       query: Application.get_env(:openagents_runtime, :dns_cluster_query) || :ignore},
      {Phoenix.PubSub, name: OpenAgentsRuntime.PubSub},
      OpenAgentsRuntime.RuntimeSupervisor,
      OpenAgentsRuntimeWeb.Endpoint
    ]

    opts = [strategy: :one_for_one, name: OpenAgentsRuntime.Supervisor]
    Supervisor.start_link(children, opts)
  end

  @impl true
  def config_change(changed, _new, removed) do
    OpenAgentsRuntimeWeb.Endpoint.config_change(changed, removed)
    :ok
  end
end
