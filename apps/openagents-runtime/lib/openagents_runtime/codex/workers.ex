defmodule OpenAgentsRuntime.Codex.Workers do
  @moduledoc """
  Persistence, ownership, and process orchestration for Codex workers.
  """

  import Ecto.Query

  alias Ecto.Multi
  alias OpenAgentsRuntime.Codex.Worker
  alias OpenAgentsRuntime.Codex.WorkerEvent
  alias OpenAgentsRuntime.Codex.WorkerProcess
  alias OpenAgentsRuntime.Codex.WorkerSupervisor
  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Security.Sanitizer

  @all_topic "runtime:codex_workers"

  @type principal :: %{optional(:user_id) => integer(), optional(:guest_scope) => String.t()}

  @spec create_worker(map(), principal(), keyword()) ::
          {:ok, %{worker: Worker.t(), idempotent_replay: boolean()}} | {:error, term()}
  def create_worker(attrs, principal, opts \\ []) when is_map(attrs) and is_map(principal) do
    attrs = stringify_keys(attrs)

    with {:ok, principal} <- normalize_principal(principal) do
      worker_id = normalize_string(attrs["worker_id"]) || generated_worker_id()

      case Repo.get(Worker, worker_id) do
        %Worker{} = worker ->
          if owner_matches?(worker, principal) do
            :ok = maybe_start_worker_process(worker, opts)
            {:ok, %{worker: worker, idempotent_replay: true}}
          else
            {:error, :forbidden}
          end

        nil ->
          now = DateTime.utc_now()

          payload = %{
            worker_id: worker_id,
            owner_user_id: principal[:user_id],
            owner_guest_scope: principal[:guest_scope],
            workspace_ref: normalize_string(attrs["workspace_ref"]),
            codex_home_ref: normalize_string(attrs["codex_home_ref"]),
            adapter: normalize_string(attrs["adapter"]) || "in_memory",
            status: "running",
            latest_seq: 0,
            metadata: stringify_keys(attrs["metadata"]) || %{},
            started_at: now,
            last_heartbeat_at: now
          }

          case Repo.insert(Worker.changeset(%Worker{}, payload)) do
            {:ok, worker} ->
              :ok = maybe_start_worker_process(worker, opts)
              _ = append_event(worker.worker_id, "worker.started", %{"status" => "running"})
              {:ok, %{worker: worker, idempotent_replay: false}}

            {:error, changeset} ->
              {:error, changeset}
          end
      end
    end
  end

  @spec snapshot(String.t(), principal()) :: {:ok, map()} | {:error, term()}
  def snapshot(worker_id, principal) when is_binary(worker_id) and is_map(principal) do
    with {:ok, worker} <- fetch_authorized_worker(worker_id, principal) do
      {:ok,
       %{
         "worker_id" => worker.worker_id,
         "status" => worker.status,
         "latest_seq" => worker.latest_seq,
         "workspace_ref" => worker.workspace_ref,
         "codex_home_ref" => worker.codex_home_ref,
         "adapter" => worker.adapter,
         "metadata" => worker.metadata || %{},
         "started_at" => maybe_iso8601(worker.started_at),
         "stopped_at" => maybe_iso8601(worker.stopped_at),
         "updated_at" => maybe_iso8601(worker.updated_at)
       }}
    end
  end

  @spec submit_request(String.t(), principal(), map(), keyword()) ::
          {:ok, map()} | {:error, term()}
  def submit_request(worker_id, principal, request, opts \\ [])
      when is_binary(worker_id) and is_map(principal) and is_map(request) do
    with {:ok, worker} <- fetch_authorized_worker(worker_id, principal),
         {:ok, request} <- validate_request(request),
         {:ok, _pid} <- ensure_worker_process(worker, opts),
         {:ok, _event} <-
           append_event(worker_id, "worker.request.received", %{
             "request_id" => request["request_id"],
             "method" => request["method"]
           }) do
      case WorkerProcess.request(worker_id, request, Keyword.get(opts, :timeout, 15_000)) do
        {:ok, response} ->
          _ =
            append_event(worker_id, "worker.response", %{
              "request_id" => request["request_id"],
              "response" => Sanitizer.sanitize(response)
            })

          {:ok,
           %{
             "worker_id" => worker_id,
             "request_id" => request["request_id"],
             "ok" => true,
             "response" => response
           }}

        {:error, response} ->
          _ =
            append_event(worker_id, "worker.error", %{
              "request_id" => request["request_id"],
              "response" => Sanitizer.sanitize(response)
            })

          {:ok,
           %{
             "worker_id" => worker_id,
             "request_id" => request["request_id"],
             "ok" => false,
             "response" => response
           }}
      end
    end
  end

  @spec stop_worker(String.t(), principal(), keyword()) :: {:ok, map()} | {:error, term()}
  def stop_worker(worker_id, principal, opts \\ [])
      when is_binary(worker_id) and is_map(principal) do
    with {:ok, worker} <- fetch_authorized_worker(worker_id, principal) do
      if worker.status == "stopped" do
        {:ok, %{"worker_id" => worker_id, "status" => worker.status, "idempotent_replay" => true}}
      else
        reason = normalize_string(opts[:reason]) || "stop_requested"
        now = DateTime.utc_now()

        _ = safe_stop(worker.worker_id, reason)

        worker =
          worker
          |> Ecto.Changeset.change(status: "stopped", stopped_at: now, last_heartbeat_at: now)
          |> Repo.update!()

        _ = append_event(worker.worker_id, "worker.stopped", %{"reason" => reason})

        {:ok,
         %{
           "worker_id" => worker.worker_id,
           "status" => worker.status,
           "idempotent_replay" => false
         }}
      end
    end
  end

  @spec append_event(String.t(), String.t(), map()) :: {:ok, WorkerEvent.t()} | {:error, term()}
  def append_event(worker_id, event_type, payload)
      when is_binary(worker_id) and is_binary(event_type) and is_map(payload) do
    payload = Sanitizer.sanitize(payload)

    multi =
      Multi.new()
      |> Multi.run(:next_seq, fn repo, _changes ->
        sql = """
        UPDATE runtime.codex_workers
        SET latest_seq = latest_seq + 1,
            last_heartbeat_at = NOW()
        WHERE worker_id = $1
        RETURNING latest_seq
        """

        case repo.query(sql, [worker_id]) do
          {:ok, %{rows: [[next_seq]]}} when is_integer(next_seq) -> {:ok, next_seq}
          {:ok, %{rows: []}} -> {:error, :worker_not_found}
          {:error, reason} -> {:error, reason}
        end
      end)
      |> Multi.insert(:event, fn %{next_seq: next_seq} ->
        WorkerEvent.changeset(%WorkerEvent{}, %{
          worker_id: worker_id,
          seq: next_seq,
          event_type: event_type,
          payload: payload
        })
      end)

    case Repo.transaction(multi) do
      {:ok, %{event: event}} ->
        broadcast_event(worker_id, event.seq)
        {:ok, event}

      {:error, :next_seq, :worker_not_found, _changes} ->
        {:error, :worker_not_found}

      {:error, :event, %Ecto.Changeset{} = changeset, _changes} ->
        {:error, changeset}

      {:error, _step, reason, _changes} ->
        {:error, reason}
    end
  end

  @spec list_after(String.t(), non_neg_integer()) :: [WorkerEvent.t()]
  def list_after(worker_id, seq) when is_binary(worker_id) and is_integer(seq) and seq >= 0 do
    query =
      from(event in WorkerEvent,
        where: event.worker_id == ^worker_id and event.seq > ^seq,
        order_by: [asc: event.seq]
      )

    Repo.all(query)
  end

  @spec latest_seq(String.t()) :: non_neg_integer()
  def latest_seq(worker_id) when is_binary(worker_id) do
    case Repo.get(Worker, worker_id) do
      %Worker{latest_seq: latest_seq} when is_integer(latest_seq) and latest_seq >= 0 ->
        latest_seq

      _ ->
        0
    end
  end

  @spec oldest_seq(String.t()) :: non_neg_integer()
  def oldest_seq(worker_id) when is_binary(worker_id) do
    query =
      from(event in WorkerEvent,
        where: event.worker_id == ^worker_id,
        select: min(event.seq)
      )

    Repo.one(query) || 0
  end

  @spec subscribe(String.t()) :: :ok | {:error, term()}
  def subscribe(worker_id) when is_binary(worker_id) do
    Phoenix.PubSub.subscribe(OpenAgentsRuntime.PubSub, worker_topic(worker_id))
  end

  @spec worker_topic(String.t()) :: String.t()
  def worker_topic(worker_id), do: "runtime:codex_workers:" <> worker_id

  @spec all_topic() :: String.t()
  def all_topic, do: @all_topic

  @spec normalize_principal(map()) :: {:ok, principal()} | {:error, :invalid_principal}
  def normalize_principal(%{user_id: user_id} = principal)
      when is_integer(user_id) and user_id > 0 do
    {:ok, Map.take(principal, [:user_id, :guest_scope])}
  end

  def normalize_principal(%{guest_scope: guest_scope} = principal)
      when is_binary(guest_scope) and guest_scope != "" do
    {:ok, Map.take(principal, [:user_id, :guest_scope])}
  end

  def normalize_principal(_), do: {:error, :invalid_principal}

  defp validate_request(request) do
    request = stringify_keys(request)

    request_id =
      normalize_string(request["request_id"]) || normalize_string(request["id"]) ||
        generated_request_id()

    method = normalize_string(request["method"])
    params = stringify_keys(request["params"]) || %{}

    cond do
      is_nil(method) ->
        {:error, :invalid_request}

      true ->
        {:ok,
         %{"request_id" => request_id, "id" => request_id, "method" => method, "params" => params}}
    end
  end

  defp fetch_authorized_worker(worker_id, principal) do
    with {:ok, principal} <- normalize_principal(principal) do
      case Repo.get(Worker, worker_id) do
        %Worker{} = worker ->
          if owner_matches?(worker, principal) do
            {:ok, worker}
          else
            {:error, :forbidden}
          end

        nil ->
          {:error, :not_found}
      end
    end
  end

  defp owner_matches?(worker, principal) do
    cond do
      is_integer(principal[:user_id]) -> worker.owner_user_id == principal[:user_id]
      is_binary(principal[:guest_scope]) -> worker.owner_guest_scope == principal[:guest_scope]
      true -> false
    end
  end

  defp ensure_worker_process(worker, opts) do
    worker_opts =
      []
      |> maybe_put_opt(:adapter_module, Keyword.get(opts, :adapter_module))
      |> maybe_put_opt(:channel_capacity, Keyword.get(opts, :channel_capacity, 128))

    WorkerSupervisor.ensure_worker(worker, worker_opts)
  end

  defp maybe_start_worker_process(worker, opts) do
    case ensure_worker_process(worker, opts) do
      {:ok, _pid} -> :ok
      {:error, _reason} -> :ok
    end
  end

  defp safe_stop(worker_id, reason) do
    try do
      WorkerProcess.stop(worker_id, reason)
    catch
      _, _ -> :ok
    end
  end

  defp broadcast_event(worker_id, seq) do
    message = {:codex_worker_event_notification, %{worker_id: worker_id, seq: seq}}
    Phoenix.PubSub.broadcast(OpenAgentsRuntime.PubSub, @all_topic, message)
    Phoenix.PubSub.broadcast(OpenAgentsRuntime.PubSub, worker_topic(worker_id), message)
    :ok
  end

  defp generated_worker_id do
    "codexw_" <> Integer.to_string(System.unique_integer([:positive]))
  end

  defp generated_request_id do
    "req_" <> Integer.to_string(System.unique_integer([:positive]))
  end

  defp maybe_iso8601(nil), do: nil
  defp maybe_iso8601(%DateTime{} = datetime), do: DateTime.to_iso8601(datetime)

  defp normalize_string(value) when is_binary(value) do
    value
    |> String.trim()
    |> case do
      "" -> nil
      trimmed -> trimmed
    end
  end

  defp normalize_string(_), do: nil

  defp stringify_keys(nil), do: %{}

  defp stringify_keys(map) when is_map(map) do
    Map.new(map, fn
      {key, value} when is_atom(key) -> {Atom.to_string(key), value}
      {key, value} -> {to_string(key), value}
    end)
  end

  defp maybe_put_opt(opts, _key, nil), do: opts
  defp maybe_put_opt(opts, key, value), do: Keyword.put(opts, key, value)
end
