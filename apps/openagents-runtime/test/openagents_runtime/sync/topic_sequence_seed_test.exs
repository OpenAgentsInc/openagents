defmodule OpenAgentsRuntime.Sync.TopicSequenceSeedTest do
  use OpenAgentsRuntime.DataCase, async: true

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Sync.TopicSequence

  @tag :sync_watermarks
  test "known khala topics are seeded with zero watermark" do
    rows =
      from(sequence in TopicSequence,
        select: {sequence.topic, sequence.next_watermark}
      )
      |> Repo.all()

    topics = rows |> Enum.map(&elem(&1, 0)) |> MapSet.new()

    assert topics ==
             MapSet.new([
               "runtime.run_summaries",
               "runtime.codex_worker_summaries",
               "runtime.codex_worker_events",
               "runtime.notifications"
             ])

    assert Enum.all?(rows, fn {_topic, watermark} -> watermark == 0 end)
  end
end
