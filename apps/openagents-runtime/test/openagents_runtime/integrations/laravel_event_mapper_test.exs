defmodule OpenAgentsRuntime.Integrations.LaravelEventMapperTest do
  use ExUnit.Case, async: true

  alias OpenAgentsRuntime.Integrations.LaravelEventMapper

  @fixtures_path Path.expand("../../fixtures/laravel_event_mapper_cases.json", __DIR__)

  test "matches golden mapping fixtures" do
    cases = @fixtures_path |> File.read!() |> Jason.decode!()

    Enum.each(cases, fn fixture ->
      input = fixture["input"]

      actual =
        LaravelEventMapper.map_runtime_event(
          input["run_id"],
          input["seq"],
          input["event_type"],
          input["payload"]
        )
        |> normalize_frames()

      assert actual == fixture["expected"], "fixture #{fixture["name"]} mismatch"
    end)
  end

  test "renders SSE chunks with event/id/data lines" do
    frame = %{
      event: "message",
      id: "42",
      data: Jason.encode!(%{"type" => "text-delta", "delta" => "abc"})
    }

    chunk = LaravelEventMapper.to_sse_chunk(frame)

    assert chunk =~ "event: message\n"
    assert chunk =~ "id: 42\n"
    assert chunk =~ "data: "
    assert chunk =~ "\"type\":\"text-delta\""
    assert chunk =~ "\"delta\":\"abc\""
    assert chunk =~ "\n\n"
  end

  defp normalize_frames(frames) do
    Enum.map(frames, fn frame ->
      %{
        "event" => frame.event,
        "id" => frame.id,
        "data" => decode_data(frame.data)
      }
    end)
  end

  defp decode_data("[DONE]"), do: "[DONE]"

  defp decode_data(data) do
    case Jason.decode(data) do
      {:ok, value} -> value
      {:error, _} -> data
    end
  end
end
