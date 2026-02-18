defmodule OpenAgentsRuntime.Runs.EventNotifier do
  @moduledoc """
  Transactional Postgres notification helpers for run event append wakeups.
  """

  @channel "runtime_run_events"

  @spec channel() :: String.t()
  def channel, do: @channel

  @spec payload(String.t(), non_neg_integer()) :: String.t()
  def payload(run_id, seq) when is_binary(run_id) and is_integer(seq) and seq >= 0 do
    Jason.encode!(%{"run_id" => run_id, "seq" => seq})
  end

  @spec notify(Ecto.Repo.t(), String.t(), non_neg_integer()) ::
          {:ok, :notified} | {:error, term()}
  def notify(repo, run_id, seq) do
    sql = "SELECT pg_notify($1, $2)"

    case repo.query(sql, [channel(), payload(run_id, seq)]) do
      {:ok, _result} -> {:ok, :notified}
      {:error, reason} -> {:error, reason}
    end
  end

  @spec decode_payload(String.t()) ::
          {:ok, %{run_id: String.t(), seq: non_neg_integer()}} | {:error, :invalid}
  def decode_payload(payload) when is_binary(payload) do
    with {:ok, %{"run_id" => run_id, "seq" => seq}} <- Jason.decode(payload),
         true <- is_binary(run_id),
         true <- is_integer(seq) and seq >= 0 do
      {:ok, %{run_id: run_id, seq: seq}}
    else
      _ -> {:error, :invalid}
    end
  end
end
