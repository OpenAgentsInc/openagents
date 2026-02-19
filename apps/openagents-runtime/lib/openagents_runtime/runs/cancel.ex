defmodule OpenAgentsRuntime.Runs.Cancel do
  @moduledoc """
  Durable run cancellation boundary.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Runs.RunEvents

  @terminal_statuses MapSet.new(["canceled", "succeeded", "failed"])

  @type cancel_result :: %{idempotent_replay: boolean(), status: String.t()}

  @spec request_cancel(String.t(), map()) :: {:ok, cancel_result()} | {:error, term()}
  def request_cancel(run_id, attrs \\ %{}) when is_binary(run_id) and is_map(attrs) do
    reason = attrs[:reason] || attrs["reason"] || "cancel requested"
    requested_by = attrs[:requested_by] || attrs["requested_by"] || "control_plane"

    case Repo.get(Run, run_id) do
      nil ->
        {:error, :run_not_found}

      %Run{} = run ->
        cond do
          terminal_status?(run.status) ->
            {:ok, %{idempotent_replay: true, status: run.status}}

          cancel_requested?(run_id) ->
            with {:ok, run} <- maybe_mark_canceling(run) do
              {:ok, %{idempotent_replay: true, status: run.status}}
            end

          true ->
            payload = %{
              "reason" => reason,
              "requested_by" => requested_by,
              "requested_at" =>
                DateTime.utc_now() |> DateTime.truncate(:second) |> DateTime.to_iso8601()
            }

            with {:ok, _event} <- RunEvents.append_event(run_id, "run.cancel_requested", payload),
                 {:ok, run} <- maybe_mark_canceling(run) do
              {:ok, %{idempotent_replay: false, status: run.status}}
            end
        end
    end
  end

  @spec cancel_requested?(String.t()) :: boolean()
  def cancel_requested?(run_id) when is_binary(run_id) do
    query =
      from(event in RunEvent,
        where: event.run_id == ^run_id and event.event_type == "run.cancel_requested",
        select: 1,
        limit: 1
      )

    Repo.exists?(query)
  end

  defp maybe_mark_canceling(%Run{} = run) do
    if terminal_status?(run.status) do
      {:ok, run}
    else
      run
      |> Ecto.Changeset.change(%{status: "canceling"})
      |> Repo.update()
    end
  end

  defp terminal_status?(status) when is_binary(status) do
    MapSet.member?(@terminal_statuses, status)
  end

  defp terminal_status?(_), do: false
end
