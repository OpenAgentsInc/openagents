defmodule OpenAgentsRuntime.Sync.WatermarkAllocator do
  @moduledoc """
  Allocates monotonic per-topic watermarks from `runtime.sync_topic_sequences`.
  """

  alias OpenAgentsRuntime.Repo

  @update_sql """
  UPDATE runtime.sync_topic_sequences
  SET next_watermark = next_watermark + 1,
      updated_at = now()
  WHERE topic = $1
  RETURNING next_watermark
  """

  @insert_if_missing_sql """
  INSERT INTO runtime.sync_topic_sequences (topic, next_watermark, inserted_at, updated_at)
  VALUES ($1, 0, now(), now())
  ON CONFLICT (topic) DO NOTHING
  """

  @type result :: {:ok, non_neg_integer()} | {:error, term()}

  @spec next(String.t()) :: result()
  def next(topic) when is_binary(topic) do
    case normalize_topic(topic) do
      {:ok, normalized_topic} ->
        Repo.transaction(fn ->
          with :ok <- ensure_topic_exists(normalized_topic),
               {:ok, watermark} <- allocate_next_watermark(normalized_topic) do
            watermark
          else
            {:error, reason} -> Repo.rollback(reason)
          end
        end)
        |> case do
          {:ok, watermark} -> {:ok, watermark}
          {:error, reason} -> {:error, reason}
        end

      {:error, reason} ->
        {:error, reason}
    end
  end

  def next(_topic), do: {:error, :invalid_topic}

  defp normalize_topic(topic) do
    trimmed = String.trim(topic)

    if trimmed == "" do
      {:error, :invalid_topic}
    else
      {:ok, trimmed}
    end
  end

  defp ensure_topic_exists(topic) do
    case Repo.query(@insert_if_missing_sql, [topic]) do
      {:ok, _result} -> :ok
      {:error, reason} -> {:error, {:seed_insert_failed, reason}}
    end
  end

  defp allocate_next_watermark(topic) do
    case Repo.query(@update_sql, [topic]) do
      {:ok, %{rows: [[watermark]]}} when is_integer(watermark) and watermark >= 0 ->
        {:ok, watermark}

      {:ok, _result} ->
        {:error, :allocator_returned_no_watermark}

      {:error, reason} ->
        {:error, {:allocator_update_failed, reason}}
    end
  end
end
