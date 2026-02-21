defmodule OpenAgentsRuntimeWeb.Router do
  use OpenAgentsRuntimeWeb, :router

  pipeline :api do
    plug :accepts, ["json"]
  end

  pipeline :internal_api do
    plug :accepts, ["json"]
    plug OpenAgentsRuntimeWeb.Plugs.InternalAuth
    plug OpenAgentsRuntimeWeb.Plugs.LegacyWriteFreeze
  end

  # SSE endpoints should not be blocked by JSON-only content negotiation.
  pipeline :internal_stream_api do
    plug OpenAgentsRuntimeWeb.Plugs.InternalAuth
  end

  scope "/internal/v1", OpenAgentsRuntimeWeb do
    pipe_through :internal_api

    get "/health", HealthController, :show
    post "/comms/delivery-events", CommsController, :record_delivery_event
    post "/tools/execute", ToolsController, :execute
    get "/skills/tool-specs", SkillRegistryController, :list_tool_specs
    post "/skills/tool-specs", SkillRegistryController, :upsert_tool_spec
    get "/skills/skill-specs", SkillRegistryController, :list_skill_specs
    post "/skills/skill-specs", SkillRegistryController, :upsert_skill_spec
    post "/skills/skill-specs/:skill_id/:version/publish", SkillRegistryController, :publish_skill
    get "/skills/releases/:skill_id/:version", SkillRegistryController, :show_skill_release
    get "/codex/workers", CodexWorkerController, :list
    post "/codex/workers", CodexWorkerController, :create
    get "/codex/workers/:worker_id/snapshot", CodexWorkerController, :snapshot
    post "/codex/workers/:worker_id/requests", CodexWorkerController, :request
    post "/codex/workers/:worker_id/events", CodexWorkerController, :events
    post "/codex/workers/:worker_id/stop", CodexWorkerController, :stop
    get "/runs/:run_id/snapshot", RunController, :snapshot
    post "/runs/:run_id/frames", RunController, :append_frame
    post "/runs/:run_id/cancel", RunController, :cancel
  end

  scope "/internal/v1", OpenAgentsRuntimeWeb do
    pipe_through :internal_stream_api

    get "/codex/workers/:worker_id/stream", CodexWorkerController, :stream
    get "/runs/:run_id/stream", RunController, :stream
  end
end
