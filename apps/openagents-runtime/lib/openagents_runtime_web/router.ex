defmodule OpenAgentsRuntimeWeb.Router do
  use OpenAgentsRuntimeWeb, :router

  pipeline :api do
    plug :accepts, ["json"]
  end

  pipeline :internal_api do
    plug :accepts, ["json"]
    plug OpenAgentsRuntimeWeb.Plugs.InternalAuth
  end

  scope "/internal/v1", OpenAgentsRuntimeWeb do
    pipe_through :internal_api

    get "/health", HealthController, :show
    post "/comms/delivery-events", CommsController, :record_delivery_event
    get "/runs/:run_id/snapshot", RunController, :snapshot
    get "/runs/:run_id/stream", RunController, :stream
    post "/runs/:run_id/frames", RunController, :append_frame
    post "/runs/:run_id/cancel", RunController, :cancel
  end
end
