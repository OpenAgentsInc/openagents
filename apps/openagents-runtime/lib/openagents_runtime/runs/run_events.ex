defmodule OpenAgentsRuntime.Runs.RunEvents do
  @moduledoc """
  Durable event log operations with monotonic sequence allocation per run.
  """

  import Ecto.Query

  alias Ecto.Multi
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.EventHashChain
  alias OpenAgentsRuntime.Runs.EventNotifier
  alias OpenAgentsRuntime.Runs.Run
  alias OpenAgentsRuntime.Runs.RunEvent
  alias OpenAgentsRuntime.Security.Sanitizer

  @type append_error :: :run_not_found | Ecto.Changeset.t()

  @spec append_event(String.t(), String.t(), map()) ::
          {:ok, RunEvent.t()} | {:error, append_error()}
  def append_event(run_id, event_type, payload \\ %{})
      when is_binary(run_id) and is_binary(event_type) and is_map(payload) do
    sanitized_payload = Sanitizer.sanitize(payload)

    multi =
      Multi.new()
      |> Multi.run(:next_seq, fn repo, _changes ->
        with %Run{} = run <- repo.get(Run, run_id),
             {:ok, {1, [next_seq]}} <- increment_run_sequence(repo, run_id) do
          {:ok, {run, next_seq}}
        else
          nil -> {:error, :run_not_found}
          {:error, reason} -> {:error, reason}
        end
      end)
      |> Multi.run(:prev_hash, fn repo, %{next_seq: {_run, next_seq}} ->
        previous_seq = next_seq - 1

        case previous_seq do
          0 ->
            {:ok, nil}

          _ ->
            query =
              from(event in RunEvent,
                where: event.run_id == ^run_id and event.seq == ^previous_seq,
                select: event.event_hash,
                limit: 1
              )

            case repo.one(query) do
              hash when is_binary(hash) -> {:ok, hash}
              nil -> {:error, :previous_hash_missing}
            end
        end
      end)
      |> Multi.insert(:event, fn %{next_seq: {_run, next_seq}, prev_hash: prev_hash} ->
        event_hash =
          EventHashChain.hash_event(run_id, next_seq, event_type, sanitized_payload, prev_hash)

        RunEvent.changeset(%RunEvent{}, %{
          run_id: run_id,
          seq: next_seq,
          event_type: event_type,
          payload: sanitized_payload,
          prev_hash: prev_hash,
          event_hash: event_hash
        })
      end)
      |> Multi.run(:notify, fn repo, %{event: event} ->
        EventNotifier.notify(repo, event.run_id, event.seq)
      end)

    case Repo.transaction(multi) do
      {:ok, %{event: event}} -> {:ok, event}
      {:error, :next_seq, :run_not_found, _changes} -> {:error, :run_not_found}
      {:error, :event, changeset, _changes} -> {:error, changeset}
      {:error, _step, reason, _changes} -> {:error, reason}
    end
  end

  @spec list_after(String.t(), non_neg_integer()) :: [RunEvent.t()]
  def list_after(run_id, seq) when is_binary(run_id) and is_integer(seq) and seq >= 0 do
    query =
      from(event in RunEvent,
        where: event.run_id == ^run_id and event.seq > ^seq,
        order_by: [asc: event.seq]
      )

    Repo.all(query)
  end

  @spec latest_seq(String.t()) :: non_neg_integer()
  def latest_seq(run_id) when is_binary(run_id) do
    query =
      from(event in RunEvent,
        where: event.run_id == ^run_id,
        select: max(event.seq)
      )

    Repo.one(query) || 0
  end

  @spec oldest_seq(String.t()) :: non_neg_integer()
  def oldest_seq(run_id) when is_binary(run_id) do
    query =
      from(event in RunEvent,
        where: event.run_id == ^run_id,
        select: min(event.seq)
      )

    Repo.one(query) || 0
  end

  @spec sort([RunEvent.t()]) :: [RunEvent.t()]
  def sort(events) when is_list(events), do: Enum.sort_by(events, &{&1.run_id, &1.seq})

  defp increment_run_sequence(repo, run_id) do
    sql = """
    UPDATE runtime.runs
    SET latest_seq = latest_seq + 1
    WHERE run_id = $1
    RETURNING latest_seq
    """

    case repo.query(sql, [run_id]) do
      {:ok, %{rows: [[latest_seq]]}} when is_integer(latest_seq) ->
        {:ok, {1, [latest_seq]}}

      {:ok, %{rows: []}} ->
        {:error, :run_not_found}

      {:ok, other} ->
        {:error, {:unexpected_update_result, other}}

      {:error, reason} ->
        {:error, reason}
    end
  end
end
