defmodule OpenAgentsRuntime.Runs.LoopDetection do
  @moduledoc """
  Deterministic run-level loop detection aligned with OpenClaw detector classes.
  """

  import Ecto.Query

  alias OpenAgentsRuntime.Repo
  alias OpenAgentsRuntime.Runs.RunFrame

  @history_size 30
  @warning_threshold 10
  @critical_threshold 20
  @global_circuit_breaker_threshold 30

  @type detector_kind ::
          :generic_repeat | :known_poll_no_progress | :ping_pong | :global_circuit_breaker

  @type detection :: %{
          detector: detector_kind(),
          reason_code: String.t(),
          count: non_neg_integer(),
          level: :warning | :critical,
          message: String.t()
        }

  @type detect_opt ::
          {:history_size, pos_integer()}
          | {:warning_threshold, pos_integer()}
          | {:critical_threshold, pos_integer()}
          | {:global_circuit_breaker_threshold, pos_integer()}
          | {:detectors,
             %{
               optional(:generic_repeat) => boolean(),
               optional(:known_poll_no_progress) => boolean(),
               optional(:ping_pong) => boolean()
             }}

  @spec detect(String.t(), RunFrame.t(), [detect_opt()]) ::
          {:ok, detection() | nil} | {:error, term()}
  def detect(run_id, %RunFrame{} = frame, opts \\ []) when is_binary(run_id) and is_list(opts) do
    resolved = resolve_opts(opts)

    with {:ok, recent_frames} <- recent_frames(run_id, frame.id, resolved.history_size),
         signatures <- frame_signatures(recent_frames, frame) do
      detection =
        detect_global_circuit_breaker(recent_frames, frame, resolved) ||
          detect_known_poll_no_progress(recent_frames, frame, resolved) ||
          detect_ping_pong(signatures, resolved) ||
          detect_generic_repeat(signatures, resolved)

      {:ok, detection}
    end
  end

  defp resolve_opts(opts) do
    detectors = Keyword.get(opts, :detectors, %{})

    warning_threshold =
      normalize_pos_int(Keyword.get(opts, :warning_threshold), @warning_threshold)

    critical_threshold =
      normalize_pos_int(Keyword.get(opts, :critical_threshold), @critical_threshold)

    critical_threshold = max(critical_threshold, warning_threshold + 1)

    global_circuit_breaker_threshold =
      normalize_pos_int(
        Keyword.get(opts, :global_circuit_breaker_threshold),
        @global_circuit_breaker_threshold
      )
      |> max(critical_threshold + 1)

    %{
      history_size: normalize_pos_int(Keyword.get(opts, :history_size), @history_size),
      warning_threshold: warning_threshold,
      critical_threshold: critical_threshold,
      global_circuit_breaker_threshold: global_circuit_breaker_threshold,
      detectors: %{
        generic_repeat: Map.get(detectors, :generic_repeat, true),
        known_poll_no_progress: Map.get(detectors, :known_poll_no_progress, true),
        ping_pong: Map.get(detectors, :ping_pong, true)
      }
    }
  end

  defp normalize_pos_int(value, _fallback) when is_integer(value) and value > 0, do: value
  defp normalize_pos_int(_value, fallback), do: fallback

  defp recent_frames(run_id, current_frame_id, history_size)
       when is_integer(current_frame_id) and current_frame_id > 0 do
    query =
      from(frame in RunFrame,
        where: frame.run_id == ^run_id and frame.id < ^current_frame_id,
        order_by: [desc: frame.id],
        limit: ^history_size
      )

    {:ok, Repo.all(query) |> Enum.reverse()}
  end

  defp recent_frames(_run_id, _current_frame_id, _history_size), do: {:ok, []}

  defp frame_signatures(frames, frame) do
    (frames ++ [frame])
    |> Enum.map(&frame_signature/1)
  end

  defp frame_signature(%RunFrame{} = frame) do
    hash = stable_payload_hash(frame.payload || %{})
    "#{frame.frame_type}:#{hash}"
  end

  defp stable_payload_hash(payload) do
    payload
    |> stable_serialize()
    |> then(&:crypto.hash(:sha256, &1))
    |> Base.encode16(case: :lower)
  end

  defp stable_serialize(value) when is_map(value) do
    value
    |> Enum.map(fn {k, v} -> {to_string(k), stable_serialize(v)} end)
    |> Enum.sort_by(fn {k, _} -> k end)
    |> :erlang.term_to_binary()
  end

  defp stable_serialize(value) when is_list(value), do: Enum.map(value, &stable_serialize/1)
  defp stable_serialize(value), do: value

  defp detect_global_circuit_breaker(recent_frames, frame, resolved) do
    count =
      (recent_frames ++ [frame])
      |> Enum.count(&loop_candidate_frame?/1)

    if count >= resolved.global_circuit_breaker_threshold do
      %{
        detector: :global_circuit_breaker,
        reason_code: "loop_detected.no_progress",
        count: count,
        level: :critical,
        message:
          "global circuit breaker tripped (loop_candidate_count=#{count}, threshold=#{resolved.global_circuit_breaker_threshold})"
      }
    else
      nil
    end
  end

  defp detect_known_poll_no_progress(_recent_frames, _frame, %{
         detectors: %{known_poll_no_progress: false}
       }),
       do: nil

  defp detect_known_poll_no_progress(recent_frames, frame, resolved) do
    if known_poll_frame?(frame) do
      frames = recent_frames ++ [frame]
      latest_signature = known_poll_signature(frame)

      count =
        frames
        |> Enum.reverse()
        |> Enum.take_while(&(known_poll_signature(&1) == latest_signature))
        |> length()

      build_detection_if_threshold(:known_poll_no_progress, count, resolved, fn ->
        "known poll no-progress streak (count=#{count})"
      end)
    else
      nil
    end
  end

  defp detect_ping_pong(_signatures, %{detectors: %{ping_pong: false}}), do: nil

  defp detect_ping_pong(signatures, resolved) do
    case alternating_suffix_count(signatures) do
      {count, first, second} when is_binary(first) and is_binary(second) and first != second ->
        build_detection_if_threshold(:ping_pong, count, resolved, fn ->
          "ping-pong pattern detected between two frame signatures (count=#{count})"
        end)

      _ ->
        nil
    end
  end

  defp detect_generic_repeat(_signatures, %{detectors: %{generic_repeat: false}}), do: nil

  defp detect_generic_repeat(signatures, resolved) do
    case List.last(signatures) do
      nil ->
        nil

      latest ->
        count =
          signatures
          |> Enum.reverse()
          |> Enum.take_while(&(&1 == latest))
          |> length()

        build_detection_if_threshold(:generic_repeat, count, resolved, fn ->
          "generic repeat detector triggered (count=#{count})"
        end)
    end
  end

  defp build_detection_if_threshold(_detector, count, resolved, _message_fun)
       when count < resolved.warning_threshold,
       do: nil

  defp build_detection_if_threshold(detector, count, resolved, message_fun) do
    level =
      if count >= resolved.critical_threshold do
        :critical
      else
        :warning
      end

    %{
      detector: detector,
      reason_code: "loop_detected.no_progress",
      count: count,
      level: level,
      message: message_fun.()
    }
  end

  defp alternating_suffix_count(signatures) when length(signatures) < 2, do: {0, nil, nil}

  defp alternating_suffix_count(signatures) do
    reversed = Enum.reverse(signatures)

    case reversed do
      [a, b | rest] when a != b ->
        count =
          rest
          |> Enum.reduce_while({2, a, b, :a}, fn signature, {acc, sa, sb, expected} ->
            case {expected, signature} do
              {:a, ^sa} -> {:cont, {acc + 1, sa, sb, :b}}
              {:b, ^sb} -> {:cont, {acc + 1, sa, sb, :a}}
              _ -> {:halt, {acc, sa, sb, expected}}
            end
          end)
          |> elem(0)

        {count, a, b}

      _ ->
        {0, nil, nil}
    end
  end

  defp known_poll_frame?(%RunFrame{} = frame) do
    payload = normalize_map(frame.payload)
    action = payload["action"]
    tool_name = payload["tool_name"] || payload["toolName"]

    frame.frame_type in ["tool_result", "tool_status", "tool_call"] and
      (tool_name == "command_status" or (tool_name == "process" and action in ["poll", "log"]))
  end

  defp loop_candidate_frame?(%RunFrame{} = frame) do
    frame.frame_type in ["tool_result", "tool_status", "tool_call", "tool_output", "agent_action"]
  end

  defp known_poll_signature(%RunFrame{} = frame) do
    payload = normalize_map(frame.payload)

    %{
      "frame_type" => frame.frame_type,
      "tool_name" => payload["tool_name"] || payload["toolName"],
      "action" => payload["action"],
      "status" => payload["status"],
      "exit_code" => payload["exit_code"] || payload["exitCode"],
      "total_lines" => payload["total_lines"] || payload["totalLines"],
      "text" => payload["text"] || payload["output"] || payload["message"]
    }
    |> stable_payload_hash()
  end

  defp normalize_map(value) when is_map(value) do
    Map.new(value, fn
      {k, v} when is_atom(k) -> {Atom.to_string(k), v}
      {k, v} -> {to_string(k), v}
    end)
  end

  defp normalize_map(_), do: %{}
end
