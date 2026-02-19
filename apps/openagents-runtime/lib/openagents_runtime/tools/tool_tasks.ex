defmodule OpenAgentsRuntime.Tools.ToolTasks do
  @moduledoc """
  Persistence and state transitions for durable tool task lifecycle records.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Tools.ToolTask

  @valid_transitions %{
    "queued" => MapSet.new(["running", "canceled", "timed_out", "failed"]),
    "running" => MapSet.new(["streaming", "succeeded", "failed", "canceled", "timed_out"]),
    "streaming" => MapSet.new(["streaming", "succeeded", "failed", "canceled", "timed_out"]),
    "succeeded" => MapSet.new(),
    "failed" => MapSet.new(),
    "canceled" => MapSet.new(),
    "timed_out" => MapSet.new()
  }

  @type enqueue_result :: %{task: ToolTask.t(), idempotent_replay: boolean()}

  @spec enqueue(map()) :: {:ok, enqueue_result()} | {:error, term()}
  def enqueue(attrs) when is_map(attrs) do
    run_id = attrs[:run_id] || attrs["run_id"]
    tool_call_id = attrs[:tool_call_id] || attrs["tool_call_id"]

    case get_by_tool_call(run_id, tool_call_id) do
      %ToolTask{} = task ->
        {:ok, %{task: task, idempotent_replay: true}}

      nil ->
        now = attrs[:queued_at] || attrs["queued_at"] || DateTime.utc_now()

        changeset =
          ToolTask.changeset(%ToolTask{}, %{
            run_id: run_id,
            tool_call_id: tool_call_id,
            tool_name: attrs[:tool_name] || attrs["tool_name"],
            state: "queued",
            input: attrs[:input] || attrs["input"] || %{},
            metadata: attrs[:metadata] || attrs["metadata"] || %{},
            queued_at: now
          })

        case Repo.insert(changeset) do
          {:ok, task} -> {:ok, %{task: task, idempotent_replay: false}}
          {:error, reason} -> {:error, reason}
        end
    end
  end

  @spec get_by_tool_call(String.t(), String.t()) :: ToolTask.t() | nil
  def get_by_tool_call(run_id, tool_call_id)
      when is_binary(run_id) and is_binary(tool_call_id) do
    query =
      from(task in ToolTask,
        where: task.run_id == ^run_id and task.tool_call_id == ^tool_call_id,
        limit: 1
      )

    Repo.one(query)
  end

  def get_by_tool_call(_, _), do: nil

  @spec list_for_run(String.t(), keyword()) :: [ToolTask.t()]
  def list_for_run(run_id, opts \\ []) when is_binary(run_id) do
    limit = Keyword.get(opts, :limit, 100)

    query =
      from(task in ToolTask,
        where: task.run_id == ^run_id,
        order_by: [desc: task.inserted_at],
        limit: ^limit
      )

    Repo.all(query)
  end

  @spec transition(ToolTask.t(), ToolTask.state()) ::
          {:ok, ToolTask.t()} | {:error, term()}
  def transition(%ToolTask{} = task, next_state) when is_binary(next_state) do
    transition(task, next_state, %{})
  end

  @spec transition(ToolTask.t(), ToolTask.state(), map()) ::
          {:ok, ToolTask.t()} | {:error, term()}
  def transition(%ToolTask{} = task, next_state, attrs)
      when is_binary(next_state) and is_map(attrs) do
    cond do
      task.state == next_state and next_state != "streaming" ->
        {:ok, task}

      valid_transition?(task.state, next_state) ->
        now = attrs[:at] || attrs["at"] || DateTime.utc_now()
        changes = build_transition_attrs(task, next_state, attrs, now)

        task
        |> Ecto.Changeset.change(changes)
        |> Repo.update()

      true ->
        {:error, :invalid_transition}
    end
  end

  @spec transition(String.t(), String.t(), ToolTask.state()) ::
          {:ok, ToolTask.t()} | {:error, term()}
  def transition(run_id, tool_call_id, next_state)
      when is_binary(run_id) and is_binary(tool_call_id) and is_binary(next_state) do
    transition(run_id, tool_call_id, next_state, %{})
  end

  @spec transition(String.t(), String.t(), ToolTask.state(), map()) ::
          {:ok, ToolTask.t()} | {:error, term()}
  def transition(run_id, tool_call_id, next_state, attrs)
      when is_binary(run_id) and is_binary(tool_call_id) and is_map(attrs) do
    case get_by_tool_call(run_id, tool_call_id) do
      %ToolTask{} = task -> transition(task, next_state, attrs)
      nil -> {:error, :not_found}
    end
  end

  @spec valid_transition?(ToolTask.state(), ToolTask.state()) :: boolean()
  def valid_transition?(current_state, next_state)
      when is_binary(current_state) and is_binary(next_state) do
    if current_state == next_state do
      true
    else
      MapSet.member?(Map.get(@valid_transitions, current_state, MapSet.new()), next_state)
    end
  end

  defp build_transition_attrs(task, next_state, attrs, now) do
    metadata_patch = attrs[:metadata] || attrs["metadata"] || %{}
    merged_metadata = Map.merge(task.metadata || %{}, metadata_patch)

    base = %{state: next_state, metadata: merged_metadata}

    case next_state do
      "running" ->
        Map.merge(base, %{running_at: now})

      "streaming" ->
        progress = attrs[:progress] || attrs["progress"]

        base
        |> Map.put(:streaming_at, now)
        |> maybe_put(:output, progress)

      "succeeded" ->
        output = attrs[:output] || attrs["output"]

        base
        |> Map.put(:succeeded_at, now)
        |> maybe_put(:output, output)
        |> Map.put(:error_class, nil)
        |> Map.put(:error_message, nil)

      "failed" ->
        base
        |> Map.put(:failed_at, now)
        |> maybe_put(:output, attrs[:output] || attrs["output"])
        |> maybe_put(:error_class, attrs[:error_class] || attrs["error_class"])
        |> maybe_put(:error_message, attrs[:error_message] || attrs["error_message"])

      "canceled" ->
        base
        |> Map.put(:canceled_at, now)
        |> maybe_put(:error_class, attrs[:error_class] || attrs["error_class"] || "canceled")
        |> maybe_put(:error_message, attrs[:error_message] || attrs["error_message"])

      "timed_out" ->
        base
        |> Map.put(:timed_out_at, now)
        |> maybe_put(:error_class, attrs[:error_class] || attrs["error_class"] || "timeout")
        |> maybe_put(:error_message, attrs[:error_message] || attrs["error_message"])

      _ ->
        base
    end
  end

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)
end
