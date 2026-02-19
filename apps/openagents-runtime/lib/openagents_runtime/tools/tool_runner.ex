defmodule OpenAgentsRuntime.Tools.ToolRunner do
  @moduledoc """
  Executes tool work in supervised tasks with durable lifecycle/progress updates.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.RunEvents
  alias OpenAgentsRuntime.Spend.Policy
  alias OpenAgentsRuntime.Spend.Reservations
  alias OpenAgentsRuntime.Security.Sanitizer
  alias OpenAgentsRuntime.Telemetry.Events
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
    started_at = System.monotonic_time()
    timeout_ms = Keyword.get(opts, :timeout_ms, @default_timeout_ms)
    run_id = Keyword.get(opts, :run_id)
    tool_call_id = Keyword.get(opts, :tool_call_id)
    tool_name = Keyword.get(opts, :tool_name) || "unknown_tool"
    input = Keyword.get(opts, :input, %{})
    settlement_opts = settlement_opts_from_run_opts(opts)
    metadata = enrich_metadata_with_settlement(Keyword.get(opts, :metadata, %{}), settlement_opts)
    trace_context = Tracing.current()

    with {:ok, persisted_task} <-
           maybe_prepare_task(run_id, tool_call_id, tool_name, input, metadata),
         {:ok, settlement_context} <-
           maybe_prepare_settlement(run_id, tool_call_id, settlement_opts),
         {:ok, _persisted_task} <- maybe_mark_running(persisted_task),
         {:ok, _} <- maybe_append_tool_call_event(run_id, tool_call_id, tool_name, input) do
      emit_tool_lifecycle(run_id, tool_call_id, tool_name, "run", "started", "running")
      progress_callback = progress_callback(run_id, tool_call_id, tool_name)

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
          case maybe_finalize_settlement_success(settlement_context, run_id, tool_call_id, value) do
            {:ok, value} ->
              _ = maybe_mark_succeeded(run_id, tool_call_id, value)

              emit_tool_lifecycle(
                run_id,
                tool_call_id,
                tool_name,
                "terminal",
                "succeeded",
                "succeeded",
                duration_ms: elapsed_ms(started_at)
              )

              {:ok, value}

            {:error, error_class} ->
              _ =
                maybe_mark_failed(
                  run_id,
                  tool_call_id,
                  error_class,
                  sanitize("settlement finalize failed")
                )

              emit_tool_lifecycle(
                run_id,
                tool_call_id,
                tool_name,
                "terminal",
                "failed",
                "failed",
                error_class: error_class,
                duration_ms: elapsed_ms(started_at)
              )

              {:error, {:failed, error_class}}
          end

        nil ->
          _ =
            maybe_finalize_settlement_failure(
              settlement_context,
              run_id,
              tool_call_id,
              "timeout"
            )

          _ = maybe_mark_timed_out(run_id, tool_call_id)

          emit_tool_lifecycle(
            run_id,
            tool_call_id,
            tool_name,
            "terminal",
            "timeout",
            "timed_out",
            error_class: "timeout",
            duration_ms: elapsed_ms(started_at)
          )

          {:error, :timeout}

        {:exit, reason} ->
          case classify_exit(run_id, tool_call_id, reason) do
            {:error, :canceled} = error ->
              _ =
                maybe_finalize_settlement_failure(
                  settlement_context,
                  run_id,
                  tool_call_id,
                  "canceled"
                )

              emit_tool_lifecycle(
                run_id,
                tool_call_id,
                tool_name,
                "terminal",
                "canceled",
                "canceled",
                error_class: "canceled",
                duration_ms: elapsed_ms(started_at)
              )

              error

            {:error, {:failed, error_class}} = error ->
              _ =
                maybe_finalize_settlement_failure(
                  settlement_context,
                  run_id,
                  tool_call_id,
                  error_class
                )

              emit_tool_lifecycle(
                run_id,
                tool_call_id,
                tool_name,
                "terminal",
                "failed",
                "failed",
                error_class: error_class,
                duration_ms: elapsed_ms(started_at)
              )

              error
          end
      end
    else
      {:error, :already_terminal} ->
        emit_tool_lifecycle(
          run_id,
          tool_call_id,
          tool_name,
          "run",
          "rejected",
          "canceled",
          error_class: "already_terminal",
          duration_ms: elapsed_ms(started_at)
        )

        {:error, :canceled}

      {:error, reason}
      when reason in [
             :settlement_already_finalized,
             :settlement_idempotency_conflict,
             :settlement_metadata_missing,
             :settlement_over_budget,
             :settlement_reconcile_required
           ] ->
        error_class = settlement_error_class(reason)
        error_message = settlement_error_message(reason)

        policy_payload =
          emit_settlement_policy_decision(run_id, tool_call_id, reason, settlement_opts)

        _ =
          maybe_mark_failed(run_id, tool_call_id, error_class, error_message,
            policy: policy_payload
          )

        emit_tool_lifecycle(
          run_id,
          tool_call_id,
          tool_name,
          "run",
          "failed",
          "failed",
          error_class: error_class,
          duration_ms: elapsed_ms(started_at)
        )

        {:error, {:failed, error_class}}

      {:error, reason} ->
        error_class = normalize_error_class(reason)

        _ = maybe_mark_failed(run_id, tool_call_id, error_class, sanitize(inspect(reason)))

        emit_tool_lifecycle(
          run_id,
          tool_call_id,
          tool_name,
          "run",
          "failed",
          "failed",
          error_class: error_class,
          duration_ms: elapsed_ms(started_at)
        )

        {:error, {:failed, error_class}}
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
             input: sanitize(input),
             metadata: sanitize(metadata)
           }) do
        {:ok, %{task: task}} -> {:ok, task}
        {:error, reason} -> {:error, reason}
      end
    else
      {:ok, nil}
    end
  end

  defp settlement_opts_from_run_opts(opts) do
    %{
      settlement_boundary: Keyword.get(opts, :settlement_boundary, false),
      authorization_id: Keyword.get(opts, :authorization_id),
      amount_sats: Keyword.get(opts, :amount_sats),
      retry_class:
        normalize_retry_class(Keyword.get(opts, :settlement_retry_class, "safe_retry")),
      provider_idempotency_key: Keyword.get(opts, :provider_idempotency_key)
    }
  end

  defp enrich_metadata_with_settlement(metadata, %{settlement_boundary: false}), do: metadata

  defp enrich_metadata_with_settlement(metadata, settlement_opts) do
    metadata
    |> normalize_payload()
    |> Map.put("settlement_boundary", true)
    |> maybe_put("authorization_id", settlement_opts.authorization_id)
    |> maybe_put("amount_sats", settlement_opts.amount_sats)
    |> maybe_put("retry_class", settlement_opts.retry_class)
    |> maybe_put("provider_idempotency_key", settlement_opts.provider_idempotency_key)
  end

  defp maybe_prepare_settlement(_run_id, _tool_call_id, %{settlement_boundary: false}),
    do: {:ok, nil}

  defp maybe_prepare_settlement(run_id, tool_call_id, settlement_opts) do
    with :ok <- validate_settlement_preflight(run_id, tool_call_id, settlement_opts),
         {:ok, result} <-
           Reservations.reserve(
             settlement_opts.authorization_id,
             run_id,
             tool_call_id,
             settlement_opts.amount_sats,
             retry_class: settlement_opts.retry_class,
             provider_idempotency_key: settlement_opts.provider_idempotency_key,
             metadata: %{
               "settlement_boundary" => true,
               "retry_class" => settlement_opts.retry_class
             }
           ) do
      {:ok,
       %{
         authorization_id: settlement_opts.authorization_id,
         retry_class: settlement_opts.retry_class,
         provider_idempotency_key: settlement_opts.provider_idempotency_key,
         reservation_id: result.reservation.id
       }}
    else
      {:error, :already_finalized} -> {:error, :settlement_already_finalized}
      {:error, :idempotency_conflict} -> {:error, :settlement_idempotency_conflict}
      {:error, :over_budget} -> {:error, :settlement_over_budget}
      {:error, :reconcile_required} -> {:error, :settlement_reconcile_required}
      {:error, :invalid_retry_class} -> {:error, :settlement_metadata_missing}
      {:error, :invalid_amount} -> {:error, :settlement_metadata_missing}
      {:error, :authorization_not_found} -> {:error, :settlement_metadata_missing}
      {:error, :invalid_transition} -> {:error, :settlement_reconcile_required}
      :error -> {:error, :settlement_metadata_missing}
    end
  end

  defp maybe_finalize_settlement_success(nil, _run_id, _tool_call_id, value), do: {:ok, value}

  defp maybe_finalize_settlement_success(settlement_context, run_id, tool_call_id, value) do
    case extract_provider_correlation_id(value) do
      nil ->
        _ =
          Reservations.mark_reconcile_required(
            settlement_context.authorization_id,
            run_id,
            tool_call_id,
            failure_reason: "missing_provider_correlation_id"
          )

        {:error, "settlement_correlation_missing"}

      provider_correlation_id ->
        case Reservations.commit(
               settlement_context.authorization_id,
               run_id,
               tool_call_id,
               provider_correlation_id: provider_correlation_id,
               provider_idempotency_key: settlement_context.provider_idempotency_key
             ) do
          {:ok, _result} -> {:ok, value}
          {:error, :already_finalized} -> {:error, "settlement_already_finalized"}
          {:error, :reservation_not_found} -> {:error, "settlement_not_reserved"}
          {:error, :invalid_transition} -> {:error, "settlement_invalid_transition"}
          {:error, :idempotency_conflict} -> {:error, "settlement_idempotency_conflict"}
          {:error, _reason} -> {:error, "settlement_commit_failed"}
        end
    end
  end

  defp maybe_finalize_settlement_failure(nil, _run_id, _tool_call_id, _failure_reason), do: :ok

  defp maybe_finalize_settlement_failure(settlement_context, run_id, tool_call_id, failure_reason) do
    if settlement_context.retry_class == "dedupe_reconcile_required" do
      _ =
        Reservations.mark_reconcile_required(
          settlement_context.authorization_id,
          run_id,
          tool_call_id,
          failure_reason: failure_reason
        )

      :ok
    else
      _ =
        Reservations.release(
          settlement_context.authorization_id,
          run_id,
          tool_call_id,
          failure_reason: failure_reason
        )

      :ok
    end
  end

  defp validate_settlement_preflight(run_id, tool_call_id, settlement_opts) do
    cond do
      not is_binary(run_id) or String.trim(run_id) == "" ->
        :error

      not is_binary(tool_call_id) or String.trim(tool_call_id) == "" ->
        :error

      not is_binary(settlement_opts.authorization_id) or
          String.trim(settlement_opts.authorization_id) == "" ->
        :error

      not is_integer(settlement_opts.amount_sats) or settlement_opts.amount_sats <= 0 ->
        :error

      not is_binary(settlement_opts.provider_idempotency_key) or
          String.trim(settlement_opts.provider_idempotency_key) == "" ->
        :error

      true ->
        :ok
    end
  end

  defp extract_provider_correlation_id(%{} = value) do
    normalized = normalize_payload(value)

    normalized["provider_correlation_id"] || normalized["settlement_correlation_id"] ||
      normalized["correlation_id"] || normalized["message_id"]
  end

  defp extract_provider_correlation_id(_), do: nil

  defp settlement_error_class(:settlement_metadata_missing), do: "settlement_metadata_missing"
  defp settlement_error_class(:settlement_over_budget), do: "settlement_budget_exhausted"
  defp settlement_error_class(:settlement_reconcile_required), do: "dedupe_reconcile_required"
  defp settlement_error_class(:settlement_already_finalized), do: "settlement_already_finalized"

  defp settlement_error_class(:settlement_idempotency_conflict),
    do: "settlement_idempotency_conflict"

  defp settlement_error_class(_), do: "settlement_execution_failed"

  defp settlement_error_message(:settlement_metadata_missing),
    do: sanitize("settlement-boundary tool missing required metadata")

  defp settlement_error_message(:settlement_over_budget),
    do: sanitize("settlement-boundary reserve rejected: budget exhausted")

  defp settlement_error_message(:settlement_reconcile_required),
    do: sanitize("settlement-boundary retry blocked until reconciliation")

  defp settlement_error_message(:settlement_already_finalized),
    do: sanitize("settlement-boundary tool call already finalized")

  defp settlement_error_message(:settlement_idempotency_conflict),
    do: sanitize("settlement-boundary retry metadata conflicts with existing reservation")

  defp settlement_error_message(_), do: sanitize("settlement-boundary execution failed")

  defp emit_settlement_policy_decision(run_id, tool_call_id, reason, settlement_opts) do
    reason_code =
      case reason do
        :settlement_over_budget -> "policy_denied.budget_exhausted"
        :settlement_metadata_missing -> "policy_denied.authorization_missing"
        :settlement_reconcile_required -> "policy_denied.explicit_deny"
        :settlement_idempotency_conflict -> "policy_denied.explicit_deny"
        :settlement_already_finalized -> "policy_denied.explicit_deny"
        _ -> "policy_denied.explicit_deny"
      end

    attrs = %{
      authorization_id: settlement_opts.authorization_id,
      authorization_mode: "delegated_budget",
      reason_code: reason_code
    }

    case Policy.emit_denial(run_id, tool_call_id, attrs) do
      {:ok, payload} -> payload
      {:error, _reason} -> nil
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
        "input" => input |> normalize_payload() |> sanitize()
      })
    else
      {:ok, :noop}
    end
  end

  defp maybe_mark_succeeded(run_id, tool_call_id, value) do
    output = value |> normalize_payload() |> sanitize()

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
               error_message: sanitize("tool execution timed out")
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
      error_message = inspect(reason) |> sanitize()

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

  defp maybe_mark_failed(run_id, tool_call_id, error_class, error_message, opts \\ []) do
    policy_payload = Keyword.get(opts, :policy)

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
               error_message: error_message,
               policy: policy_payload
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
      |> sanitize()
      |> Map.put("tool_call_id", tool_call_id)

    RunEvents.append_event(run_id, "tool.result", payload)
  end

  defp progress_callback(run_id, tool_call_id, tool_name) do
    fn progress ->
      if is_binary(run_id) and is_binary(tool_call_id) do
        payload = normalize_payload(progress)
        payload = sanitize(payload)

        _ = ToolTasks.transition(run_id, tool_call_id, "streaming", %{progress: payload})

        _ =
          RunEvents.append_event(run_id, "tool.progress", %{
            "tool_call_id" => tool_call_id,
            "progress" => payload
          })

        emit_tool_lifecycle(
          run_id,
          tool_call_id,
          tool_name,
          "progress",
          "emitted",
          "streaming"
        )
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

  defp sanitize(value), do: Sanitizer.sanitize(value)

  defp maybe_put(map, _key, nil), do: map
  defp maybe_put(map, key, value), do: Map.put(map, key, value)

  defp normalize_retry_class(value) when is_atom(value),
    do: value |> Atom.to_string() |> String.trim()

  defp normalize_retry_class(value) when is_binary(value), do: String.trim(value)
  defp normalize_retry_class(_), do: "safe_retry"

  defp emit_tool_lifecycle(run_id, tool_call_id, tool_name, phase, result, state, opts \\ []) do
    error_class = Keyword.get(opts, :error_class, "none")
    duration_ms = Keyword.get(opts, :duration_ms, 0)

    Events.emit(
      [:openagents_runtime, :tool, :lifecycle],
      %{count: 1, duration_ms: duration_ms},
      %{
        run_id: run_id,
        tool_call_id: tool_call_id,
        tool_name: tool_name,
        phase: phase,
        result: result,
        state: state,
        error_class: error_class
      }
    )
  end

  defp elapsed_ms(started_at) when is_integer(started_at) do
    (System.monotonic_time() - started_at)
    |> System.convert_time_unit(:native, :millisecond)
    |> max(0)
  end
end
