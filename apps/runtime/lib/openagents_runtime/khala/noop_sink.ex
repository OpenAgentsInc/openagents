defmodule OpenAgentsRuntime.Khala.NoopSink do
  @moduledoc """
  Default sink for projector writes when Khala mutation endpoints are not wired.
  """

  @behaviour OpenAgentsRuntime.Khala.Sink

  @impl true
  def upsert_run_summary(_document_id, _summary, _opts), do: :ok

  @impl true
  def upsert_codex_worker_summary(_document_id, _summary, _opts), do: :ok
end
