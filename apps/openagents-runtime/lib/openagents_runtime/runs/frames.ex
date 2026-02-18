defmodule OpenAgentsRuntime.Runs.Frames do
  @moduledoc """
  Frame ingestion API with idempotent `frame_id` semantics.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.RunFrame

  @type append_result :: %{frame: RunFrame.t(), idempotent_replay: boolean()}
  @type append_error :: :run_not_found | :idempotency_conflict | Ecto.Changeset.t()

  @spec append_frame(String.t(), map()) :: {:ok, append_result()} | {:error, append_error()}
  def append_frame(run_id, attrs) when is_binary(run_id) and is_map(attrs) do
    frame_id = attrs[:frame_id] || attrs["frame_id"]
    frame_type = attrs[:frame_type] || attrs[:type] || attrs["frame_type"] || attrs["type"]
    payload = attrs[:payload] || attrs["payload"] || %{}
    occurred_at = attrs[:occurred_at] || attrs["occurred_at"]

    payload_hash = payload_hash(payload)

    changeset =
      RunFrame.changeset(%RunFrame{}, %{
        run_id: run_id,
        frame_id: frame_id,
        frame_type: frame_type,
        payload: payload,
        payload_hash: payload_hash,
        occurred_at: occurred_at
      })

    case Repo.insert(changeset,
           on_conflict: :nothing,
           conflict_target: [:run_id, :frame_id],
           returning: true
         ) do
      {:ok, %RunFrame{id: nil}} ->
        resolve_existing_frame(run_id, frame_id, payload_hash, frame_type)

      {:ok, %RunFrame{} = frame} ->
        {:ok, %{frame: frame, idempotent_replay: false}}

      {:error, %Ecto.Changeset{} = changeset} ->
        if run_not_found_changeset?(changeset),
          do: {:error, :run_not_found},
          else: {:error, changeset}
    end
  end

  @spec get_frame(String.t(), String.t()) :: RunFrame.t() | nil
  def get_frame(run_id, frame_id) when is_binary(run_id) and is_binary(frame_id) do
    query =
      from(frame in RunFrame,
        where: frame.run_id == ^run_id and frame.frame_id == ^frame_id,
        limit: 1
      )

    Repo.one(query)
  end

  @spec payload_hash(map()) :: String.t()
  def payload_hash(payload) when is_map(payload) do
    payload
    |> :erlang.term_to_binary()
    |> then(&:crypto.hash(:sha256, &1))
    |> Base.encode16(case: :lower)
  end

  defp resolve_existing_frame(run_id, frame_id, payload_hash, frame_type) do
    case get_frame(run_id, frame_id) do
      %RunFrame{} = frame ->
        if frame.payload_hash == payload_hash and frame.frame_type == frame_type do
          {:ok, %{frame: frame, idempotent_replay: true}}
        else
          {:error, :idempotency_conflict}
        end

      nil ->
        {:error, :idempotency_conflict}
    end
  end

  defp run_not_found_changeset?(changeset) do
    Enum.any?(changeset.errors, fn
      {:run_id, {_message, metadata}} -> metadata[:constraint] == :foreign
      _ -> false
    end)
  end
end
