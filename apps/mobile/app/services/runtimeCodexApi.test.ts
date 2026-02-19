import { parseSseEvents } from "./runtimeCodexApi"

test("parseSseEvents parses id/event/data records", () => {
  const raw = [
    "event: message",
    "id: 4",
    'data: {"seq":4,"event_type":"worker.event"}',
    "",
    "event: heartbeat",
    "id: 5",
    'data: {"seq":5,"event_type":"worker.heartbeat"}',
    "",
  ].join("\n")

  const events = parseSseEvents(raw)
  expect(events).toHaveLength(2)
  expect(events[0]).toMatchObject({
    id: 4,
    event: "message",
  })
  expect(events[1]).toMatchObject({
    id: 5,
    event: "heartbeat",
  })
})
