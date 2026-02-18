defmodule OpenAgentsRuntimeWeb.Router do
  use OpenAgentsRuntimeWeb, :router

  pipeline :api do
    plug :accepts, ["json"]
  end

  scope "/internal/v1", OpenAgentsRuntimeWeb do
    pipe_through :api

    get "/health", HealthController, :show
    get "/runs/:run_id/snapshot", RunController, :snapshot
    get "/runs/:run_id/stream", RunController, :stream
    post "/runs/:run_id/frames", RunController, :append_frame
  end
end
