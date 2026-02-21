defmodule OpenAgentsRuntime.Runs.StreamTailerTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Runs.StreamTailer

  test "next_backoff_ms grows and caps" do
    value_1 = StreamTailer.next_backoff_ms(50)
    value_2 = StreamTailer.next_backoff_ms(value_1)

    assert value_1 >= 100
    assert value_1 <= 1_000
    assert value_2 >= value_1
    assert value_2 <= 1_000

    capped = Enum.reduce(1..10, 900, fn _, acc -> StreamTailer.next_backoff_ms(acc) end)
    assert capped <= 1_000
  end
end
