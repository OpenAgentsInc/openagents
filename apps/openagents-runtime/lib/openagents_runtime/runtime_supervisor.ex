defmodule OpenAgentsRuntime.RuntimeSupervisor do
  @moduledoc """
  Top-level runtime process supervision for internal agent execution concerns.
  """

  use Supervisor

  @spec start_link(keyword()) :: Supervisor.on_start()
  def start_link(opts \\ []) do
    Supervisor.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @impl true
  def init(_opts) do
    children = [
      {Registry, keys: :unique, name: OpenAgentsRuntime.AgentRegistry},
      {Registry, keys: :duplicate, name: OpenAgentsRuntime.ToolTaskRegistry},
      {DynamicSupervisor, strategy: :one_for_one, name: OpenAgentsRuntime.AgentSupervisor},
      {Task.Supervisor, name: OpenAgentsRuntime.Tools.TaskSupervisor},
      OpenAgentsRuntime.Runs.EventListener,
      OpenAgentsRuntime.Runs.Janitor
    ]

    Supervisor.init(children, strategy: :one_for_one)
  end
end
