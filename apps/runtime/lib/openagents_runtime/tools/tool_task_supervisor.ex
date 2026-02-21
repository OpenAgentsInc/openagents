defmodule OpenAgentsRuntime.Tools.ToolTaskSupervisor do
  @moduledoc """
  Thin helper over the runtime task supervisor namespace.
  """

  @spec name() :: module()
  def name, do: OpenAgentsRuntime.Tools.TaskSupervisor
end
