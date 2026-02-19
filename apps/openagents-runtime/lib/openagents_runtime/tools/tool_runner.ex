defmodule OpenAgentsRuntime.Tools.ToolRunner do
  @moduledoc """
  Executes tool work in supervised tasks with durable lifecycle/progress updates.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Telemetry.Tracing
  alias OpenAgentsRuntime.Tools.ToolTask
  alias OpenAgentsRuntime.Tools.ToolTasks

  @default_timeout_ms 5_000

  @type run_result ::
          {:ok, term()}
          | {:error, :timeout}
          | {:error, :canceled}
          | {:error, {:failed, String.t()}}

  @spec run(function(), timeout() | keyword()) :: run_result()
  def run(fun, timeout_ms \\ @default_timeout_ms)

  def run(fun, timeout_ms) when is_function(fun) and is_integer(timeout_ms) do
    run(fun, timeout_ms: timeout_ms)
  end

  def run(fun, opts) when is_function(fun) and is_list(opts) do
    timeout_ms = Keyword.get(opts, :timeout_ms, @default_timeout_ms)
    run_id = Keyword.get(opts, :run_id)
    tool_call_id = Keyword.get(opts, :tool_call_id)
    tool_name = Keyword.get(opts, :tool_name)
    input = Keyword.get(opts, :input, %{})
    metadata = Keyword.get(opts, :metadata, %{})
    trace_context = Tracing.current()

    with {:ok, persisted_task} <-
           maybe_prepare_task(run_id, tool_call_id, tool_name, input, metadata),
         {:ok, _persisted_task} <- maybe_mark_running(persisted_task),
         {:ok, _} <- maybe_append_tool_call_event(run_id, tool_call_id, tool_name, input) do
      progress_callback = progress_callback(run_id, tool_call_id)

      task =
        Task.Supervisor.async_nolink(OpenAgentsRuntime.Tools.TaskSupervisor, fn ->
          register_run_task(run_id, tool_call_id)
          :ok = Tracing.put_current(trace_context)

          Tracing.with_phase_span(
            :tool,
            %{component: "tool_runner", run_id: run_id, tool_call_id: tool_call_id},
            fn ->
              invoke_tool_function(fun, progress_callback)
            end
          )
        end)

      case Task.yield(task, timeout_ms) || Task.shutdown(task, :brutal_kill) do
        {:ok, value} ->
          _ = maybe_mark_succeeded(run_id, tool_call_id, value)
          {:ok, value}

        nil ->
          _ = maybe_mark_timed_out(run_id, tool_call_id)
          {:error, :timeout}

        {:exit, reason} ->
          classify_exit(run_id, tool_call_id, reason)
      end
    else
      {:error, :already_terminal} -> {:error, :canceled}
      {:error, reason} -> {:error, {:failed, normalize_error_class(reason)}}
    end
  end

  @spec cancel_run(String.t()) :: :ok
  def cancel_run(run_id) when is_binary(run_id) do
    task_entries_for_run(run_id)
    |> Enum.each(fn {pid, _meta} -> Process.exit(pid, :kill) end)

    cancel_running_task_records(run_id)
    :ok
  end

  @spec cancel_task(String.t(), String.t()) :: :ok
  def cancel_task(run_id, tool_call_id)
      when is_binary(run_id) and is_binary(tool_call_id) do
    task_entries_for_run(run_id)
    |> Enum.filter(fn {_pid, meta} -> meta_tool_call_id(meta) == tool_call_id end)
    |> Enum.each(fn {pid, _meta} -> Process.exit(pid, :kill) end)

    _ =
      ToolTasks.transition(run_id, tool_call_id, "canceled", %{
        error_class: "canceled",
        error_message: "canceled by request"
      })

    _ = append_tool_result_event(run_id, tool_call_id, %{status: "canceled"})

    :ok
  end

  defp maybe_prepare_task(run_id, tool_call_id, tool_name, input, metadata) do
    if persist_tool_task?(run_id, tool_call_id, tool_name) do
      case ToolTasks.enqueue(%{
             run_id: run_id,
             tool_call_id: tool_call_id,
             tool_name: tool_name,
             input: input,
             metadata: metadata
           }) do
        {:ok, %{task: task}} -> {:ok, task}
        {:error, reason} -> {:error, reason}
      end
    else
      {:ok, nil}
    end
  end

  defp maybe_mark_running(nil), do: {:ok, nil}

  defp maybe_mark_running(%ToolTask{state: state} = task)
       when state in ["running", "streaming"] do
    {:ok, task}
  end

  defp maybe_mark_running(%ToolTask{state: state})
       when state in ["succeeded", "failed", "canceled", "timed_out"] do
    {:error, :already_terminal}
  end

  defp maybe_mark_running(%ToolTask{} = task) do
    ToolTasks.transition(task, "running")
  end

  defp maybe_append_tool_call_event(run_id, tool_call_id, tool_name, input) do
    if is_binary(run_id) and is_binary(tool_call_id) do
      RunEvents.append_event(run_id, "tool.call", %{
        "tool_call_id" => tool_call_id,
        "tool_name" => tool_name,
        "input" => normalize_payload(input)
      })
    else
      {:ok, :noop}
    end
  end

  defp maybe_mark_succeeded(run_id, tool_call_id, value) do
    output = normalize_payload(value)

    if is_binary(run_id) and is_binary(tool_call_id) do
      with {:ok, _task} <-
             ToolTasks.transition(run_id, tool_call_id, "succeeded", %{output: output}),
           {:ok, _event} <-
             append_tool_result_event(run_id, tool_call_id, %{status: "succeeded", output: output}) do
        :ok
      else
        _ -> :error
      end
    else
      :ok
    end
  end

  defp maybe_mark_timed_out(run_id, tool_call_id) do
    if is_binary(run_id) and is_binary(tool_call_id) do
      with {:ok, _task} <-
             ToolTasks.transition(run_id, tool_call_id, "timed_out", %{
               error_class: "timeout",
               error_message: "tool execution timed out"
             }),
           {:ok, _event} <-
             append_tool_result_event(run_id, tool_call_id, %{
               status: "timed_out",
               error_class: "timeout"
             }) do
        :ok
      else
        _ -> :error
      end
    else
      :ok
    end
  end

  defp classify_exit(run_id, tool_call_id, reason) do
    if canceled_reason?(reason) do
      _ =
        maybe_mark_canceled(run_id, tool_call_id, %{
          error_class: "canceled",
          error_message: "tool task canceled"
        })

      {:error, :canceled}
    else
      error_class = normalize_error_class(reason)
      error_message = inspect(reason)

      _ = maybe_mark_failed(run_id, tool_call_id, error_class, error_message)

      {:error, {:failed, error_class}}
    end
  end

  defp maybe_mark_canceled(run_id, tool_call_id, attrs) do
    if is_binary(run_id) and is_binary(tool_call_id) do
      with {:ok, _task} <- ToolTasks.transition(run_id, tool_call_id, "canceled", attrs),
           {:ok, _event} <- append_tool_result_event(run_id, tool_call_id, %{status: "canceled"}) do
        :ok
      else
        _ -> :error
      end
    else
      :ok
    end
  end

  defp maybe_mark_failed(run_id, tool_call_id, error_class, error_message) do
    if is_binary(run_id) and is_binary(tool_call_id) do
      with {:ok, _task} <-
             ToolTasks.transition(run_id, tool_call_id, "failed", %{
               error_class: error_class,
               error_message: error_message
             }),
           {:ok, _event} <-
             append_tool_result_event(run_id, tool_call_id, %{
               status: "failed",
               error_class: error_class,
               error_message: error_message
             }) do
        :ok
      else
        _ -> :error
      end
    else
      :ok
    end
  end

  defp append_tool_result_event(run_id, tool_call_id, attrs) do
    payload =
      attrs
      |> normalize_payload()
      |> Map.put("tool_call_id", tool_call_id)

    RunEvents.append_event(run_id, "tool.result", payload)
  end

  defp progress_callback(run_id, tool_call_id) do
    fn progress ->
      if is_binary(run_id) and is_binary(tool_call_id) do
        payload = normalize_payload(progress)

        _ = ToolTasks.transition(run_id, tool_call_id, "streaming", %{progress: payload})

        _ =
          RunEvents.append_event(run_id, "tool.progress", %{
            "tool_call_id" => tool_call_id,
            "progress" => payload
          })
      end

      :ok
    end
  end

  defp invoke_tool_function(fun, progress_callback) do
    case :erlang.fun_info(fun, :arity) do
      {:arity, 1} -> fun.(progress_callback)
      _ -> fun.()
    end
  end

  defp cancel_running_task_records(run_id) do
    query =
      from(task in ToolTask,
        where: task.run_id == ^run_id and task.state in ["queued", "running", "streaming"]
      )

    query
    |> Repo.all()
    |> Enum.each(fn task ->
      _ =
        ToolTasks.transition(task, "canceled", %{
          error_class: "canceled",
          error_message: "canceled by run request"
        })

      _ = append_tool_result_event(run_id, task.tool_call_id, %{status: "canceled"})
    end)
  end

  defp register_run_task(run_id, tool_call_id) when is_binary(run_id) do
    Registry.register(OpenAgentsRuntime.ToolTaskRegistry, run_id, %{tool_call_id: tool_call_id})
    :ok
  end

  defp register_run_task(_, _), do: :ok

  defp task_entries_for_run(run_id) do
    OpenAgentsRuntime.ToolTaskRegistry
    |> Registry.lookup(run_id)
    |> Enum.uniq_by(fn {pid, _meta} -> pid end)
  end

  defp meta_tool_call_id(%{tool_call_id: tool_call_id}) when is_binary(tool_call_id),
    do: tool_call_id

  defp meta_tool_call_id(_), do: nil

  defp persist_tool_task?(run_id, tool_call_id, tool_name) do
    is_binary(run_id) and is_binary(tool_call_id) and is_binary(tool_name)
  end

  defp canceled_reason?(:killed), do: true
  defp canceled_reason?(:shutdown), do: true
  defp canceled_reason?({:shutdown, _}), do: true
  defp canceled_reason?(_), do: false

  defp normalize_payload(value) when is_map(value) do
    Map.new(value, fn
      {key, val} when is_atom(key) -> {Atom.to_string(key), normalize_payload(val)}
      {key, val} -> {to_string(key), normalize_payload(val)}
    end)
  end

  defp normalize_payload(value) when is_list(value), do: Enum.map(value, &normalize_payload/1)
  defp normalize_payload(value), do: value

  defp normalize_error_class({exception, _stacktrace}) when is_struct(exception),
    do: exception.__struct__ |> Module.split() |> List.last() |> Macro.underscore()

  defp normalize_error_class(exception) when is_struct(exception),
    do: exception.__struct__ |> Module.split() |> List.last() |> Macro.underscore()

  defp normalize_error_class(reason) when is_atom(reason), do: Atom.to_string(reason)
  defp normalize_error_class(reason) when is_binary(reason), do: reason
  defp normalize_error_class(_), do: "tool_execution_failed"
end
