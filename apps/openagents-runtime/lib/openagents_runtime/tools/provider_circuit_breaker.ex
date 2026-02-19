defmodule OpenAgentsRuntime.Tools.ProviderCircuitBreaker do
  @moduledoc """
  Lightweight in-memory circuit breaker for provider calls.
  """

  alias OpenAgentsRuntime.Telemetry.Events

  @table :openagents_runtime_provider_breakers
  @states [:closed, :open, :half_open]
  @default_failure_threshold 3
  @default_reset_timeout_ms 30_000

  @type provider :: String.t()
  @type breaker_state :: :closed | :open | :half_open

  @type state_snapshot :: %{
          state: breaker_state(),
          failure_count: non_neg_integer(),
          opened_at_ms: integer() | nil
        }

  @spec call(provider(), (-> term()), keyword()) :: term() | {:error, :circuit_open}
  def call(provider, fun, opts \\ []) when is_binary(provider) and is_function(fun, 0) do
    ensure_table!()

    failure_threshold = Keyword.get(opts, :failure_threshold, @default_failure_threshold)
    reset_timeout_ms = Keyword.get(opts, :reset_timeout_ms, @default_reset_timeout_ms)
    now_ms = now_ms()

    state = get_state(provider)

    case state.state do
      :open ->
        if open_window_elapsed?(state, now_ms, reset_timeout_ms) do
          half_open = %{state | state: :half_open}
          persist_state(provider, half_open)
          emit_state(provider, half_open.state, "half_open")
          run_provider_call(provider, half_open, fun, failure_threshold, now_ms)
        else
          emit_state(provider, state.state, "short_circuit")
          {:error, :circuit_open}
        end

      _ ->
        run_provider_call(provider, state, fun, failure_threshold, now_ms)
    end
  end

  @spec current_state(provider()) :: state_snapshot()
  def current_state(provider) when is_binary(provider) do
    ensure_table!()
    get_state(provider)
  end

  @spec reset(provider() | :all) :: :ok
  def reset(:all) do
    ensure_table!()
    :ets.delete_all_objects(@table)
    :ok
  end

  def reset(provider) when is_binary(provider) do
    ensure_table!()
    :ets.delete(@table, provider)
    :ok
  end

  defp run_provider_call(provider, state, fun, failure_threshold, now_ms) do
    result = safe_invoke(fun)

    if provider_failure?(result) do
      next_state = on_failure(state, failure_threshold, now_ms)
      persist_state(provider, next_state)
      emit_state(provider, next_state.state, "failure")
      result
    else
      next_state = on_success(state)
      persist_state(provider, next_state)
      emit_state(provider, next_state.state, "success")
      result
    end
  end

  defp on_failure(%{state: :half_open} = state, _threshold, now_ms) do
    %{state | state: :open, failure_count: max(state.failure_count, 1), opened_at_ms: now_ms}
  end

  defp on_failure(%{state: :closed} = state, threshold, now_ms) do
    failure_count = state.failure_count + 1

    if failure_count >= threshold do
      %{state | state: :open, failure_count: failure_count, opened_at_ms: now_ms}
    else
      %{state | failure_count: failure_count}
    end
  end

  defp on_failure(%{state: :open} = state, _threshold, _now_ms), do: state

  defp on_success(%{state: :half_open} = state) do
    %{state | state: :closed, failure_count: 0, opened_at_ms: nil}
  end

  defp on_success(%{state: :closed} = state) do
    %{state | failure_count: 0, opened_at_ms: nil}
  end

  defp on_success(%{state: :open} = state), do: state

  defp provider_failure?({:error, _reason}), do: true
  defp provider_failure?(_result), do: false

  defp safe_invoke(fun) do
    fun.()
  rescue
    exception ->
      {:error, {:exception, exception, __STACKTRACE__}}
  catch
    kind, value ->
      {:error, {kind, value}}
  end

  defp get_state(provider) do
    case :ets.lookup(@table, provider) do
      [{^provider, state}] -> state
      [] -> default_state()
    end
  end

  defp persist_state(provider, state) do
    :ets.insert(@table, {provider, state})
    :ok
  end

  defp open_window_elapsed?(state, now_ms, reset_timeout_ms) do
    opened_at_ms = state.opened_at_ms || 0
    now_ms - opened_at_ms >= reset_timeout_ms
  end

  defp emit_state(provider, current_state, event) do
    Enum.each(@states, fn state ->
      value = if state == current_state, do: 1, else: 0

      Events.emit(
        [:openagents_runtime, :provider, :breaker, :state],
        %{state: value},
        %{
          provider: provider,
          state: Atom.to_string(state),
          event: event
        }
      )
    end)
  end

  defp ensure_table! do
    case :ets.whereis(@table) do
      :undefined ->
        :ets.new(@table, [:set, :named_table, :public, read_concurrency: true])
        :ok

      _tid ->
        :ok
    end
  end

  defp default_state do
    %{
      state: :closed,
      failure_count: 0,
      opened_at_ms: nil
    }
  end

  defp now_ms, do: System.monotonic_time(:millisecond)
end
