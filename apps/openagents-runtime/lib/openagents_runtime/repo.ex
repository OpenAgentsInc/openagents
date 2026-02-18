defmodule OpenAgentsRuntime.Repo do
  use Ecto.Repo,
    otp_app: :openagents_runtime,
    adapter: Ecto.Adapters.Postgres
end
