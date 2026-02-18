defmodule OpenAgentsRuntimeWeb.Router do
  use OpenAgentsRuntimeWeb, :router

  pipeline :api do
    plug :accepts, ["json"]
  end

  scope "/internal/v1", OpenAgentsRuntimeWeb do
    pipe_through :api

    get "/health", HealthController, :show
  end
end
