defmodule OpenAgentsRuntimeWeb.HealthController do
  use OpenAgentsRuntimeWeb, :controller

  @spec show(Plug.Conn.t(), map()) :: Plug.Conn.t()
  def show(conn, _params) do
    json(conn, %{
      status: "ok",
      service: "openagents-runtime",
      version: to_string(Application.spec(:openagents_runtime, :vsn) || "dev")
    })
  end
end
