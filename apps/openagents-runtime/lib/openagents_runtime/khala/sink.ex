defmodule OpenAgentsRuntime.Khala.Sink do
  @moduledoc """
  Behaviour for runtime-owned Khala projection writes.

  Sink implementations must upsert projector-owned document ids only.
  """

  @callback upsert_run_summary(document_id :: String.t(), summary :: map(), opts :: keyword()) ::
              :ok | {:error, term()}

  @callback upsert_codex_worker_summary(
              document_id :: String.t(),
              summary :: map(),
              opts :: keyword()
            ) :: :ok | {:error, term()}
end
