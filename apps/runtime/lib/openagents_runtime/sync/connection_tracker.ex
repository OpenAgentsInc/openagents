defmodule OpenAgentsRuntime.Sync.ConnectionTracker do
  @moduledoc """
  Tracks active Khala sync socket connections.
  """

  @table :openagents_runtime_sync_connection_tracker
  @key :active_connections

  @spec increment() :: non_neg_integer()
  def increment do
    ensure_table()
    :ets.update_counter(@table, @key, {2, 1}, {@key, 0})
  end

  @spec decrement() :: non_neg_integer()
  def decrement do
    ensure_table()
    :ets.update_counter(@table, @key, {2, -1, 0, 0}, {@key, 0})
  end

  @spec current() :: non_neg_integer()
  def current do
    ensure_table()

    case :ets.lookup(@table, @key) do
      [{@key, value}] when is_integer(value) and value >= 0 -> value
      _ -> 0
    end
  end

  @spec reset_for_tests() :: :ok
  def reset_for_tests do
    ensure_table()
    true = :ets.insert(@table, {@key, 0})
    :ok
  end

  defp ensure_table do
    case :ets.whereis(@table) do
      :undefined ->
        :ets.new(@table, [:named_table, :public, :set, read_concurrency: true])

      table ->
        table
    end
  rescue
    ArgumentError ->
      @table
  end
end
