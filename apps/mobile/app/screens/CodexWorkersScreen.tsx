import { FC, useCallback, useEffect, useMemo, useState } from "react"
import { FlatList, TextStyle, View, ViewStyle } from "react-native"

import { Button } from "@/components/Button"
import { Screen } from "@/components/Screen"
import { Text } from "@/components/Text"
import { TextField } from "@/components/TextField"
import { useAuth } from "@/context/AuthContext"
import { DemoTabScreenProps } from "@/navigators/navigationTypes"
import {
  RuntimeCodexApiError,
  RuntimeCodexStreamEvent,
  RuntimeCodexWorkerSnapshot,
  RuntimeCodexWorkerStatus,
  RuntimeCodexWorkerSummary,
  getRuntimeCodexWorkerSnapshot,
  listRuntimeCodexWorkers,
  requestRuntimeCodexWorker,
  stopRuntimeCodexWorker,
  streamRuntimeCodexWorker,
} from "@/services/runtimeCodexApi"
import { useAppTheme } from "@/theme/context"
import type { ThemedStyle } from "@/theme/types"

const STREAM_LIMIT = 80
const LIST_REFRESH_MS = 12_000
const STREAM_RETRY_MS = 2_000

type WorkerFilter = "all" | RuntimeCodexWorkerStatus

export const CodexWorkersScreen: FC<DemoTabScreenProps<"Codex">> = function CodexWorkersScreen() {
  const { authToken, isAuthenticated } = useAuth()
  const { themed } = useAppTheme()

  const [filter, setFilter] = useState<WorkerFilter>("all")
  const [workers, setWorkers] = useState<RuntimeCodexWorkerSummary[]>([])
  const [workersLoading, setWorkersLoading] = useState(false)
  const [workersError, setWorkersError] = useState<string | null>(null)

  const [selectedWorkerId, setSelectedWorkerId] = useState<string | null>(null)
  const [snapshot, setSnapshot] = useState<RuntimeCodexWorkerSnapshot | null>(null)
  const [snapshotLoading, setSnapshotLoading] = useState(false)
  const [snapshotError, setSnapshotError] = useState<string | null>(null)

  const [streamEvents, setStreamEvents] = useState<RuntimeCodexStreamEvent[]>([])
  const [streamState, setStreamState] = useState<"idle" | "live" | "reconnecting">("idle")
  const [streamError, setStreamError] = useState<string | null>(null)

  const [requestMethod, setRequestMethod] = useState("thread/list")
  const [requestParams, setRequestParams] = useState("{}")
  const [requestBusy, setRequestBusy] = useState(false)
  const [stopBusy, setStopBusy] = useState(false)
  const [adminDenied, setAdminDenied] = useState(false)
  const [lastAction, setLastAction] = useState<string | null>(null)

  const selectedWorkerSummary = useMemo(
    () => workers.find((worker) => worker.worker_id === selectedWorkerId) ?? null,
    [workers, selectedWorkerId],
  )

  const loadWorkers = useCallback(
    async (silent = false) => {
      if (!authToken) return
      if (!silent) setWorkersLoading(true)
      setWorkersError(null)

      try {
        const result = await listRuntimeCodexWorkers(authToken, filter)
        setWorkers(result)
        setSelectedWorkerId((current) => {
          if (current && result.some((worker) => worker.worker_id === current)) return current
          return result.length > 0 ? result[0].worker_id : null
        })
      } catch (error) {
        const message =
          error instanceof RuntimeCodexApiError ? error.message : "Failed to load workers."
        setWorkersError(message)
      } finally {
        if (!silent) setWorkersLoading(false)
      }
    },
    [authToken, filter],
  )

  const loadSnapshot = useCallback(
    async (workerId: string, silent = false) => {
      if (!authToken) return
      if (!silent) setSnapshotLoading(true)
      setSnapshotError(null)

      try {
        const response = await getRuntimeCodexWorkerSnapshot(authToken, workerId)
        setSnapshot(response)
      } catch (error) {
        const message =
          error instanceof RuntimeCodexApiError ? error.message : "Failed to load worker snapshot."
        setSnapshotError(message)
      } finally {
        if (!silent) setSnapshotLoading(false)
      }
    },
    [authToken],
  )

  useEffect(() => {
    if (!authToken) {
      setWorkers([])
      setSelectedWorkerId(null)
      setSnapshot(null)
      setStreamEvents([])
      setWorkersError(null)
      setSnapshotError(null)
      setStreamError(null)
      setStreamState("idle")
      return
    }

    void loadWorkers()
  }, [authToken, loadWorkers])

  useEffect(() => {
    if (!authToken) return
    const handle = setInterval(() => {
      void loadWorkers(true)
    }, LIST_REFRESH_MS)

    return () => clearInterval(handle)
  }, [authToken, loadWorkers])

  useEffect(() => {
    if (!selectedWorkerId) {
      setSnapshot(null)
      setStreamEvents([])
      setStreamError(null)
      setStreamState("idle")
      return
    }

    setStreamEvents([])
    setStreamError(null)
    setStreamState("idle")
    void loadSnapshot(selectedWorkerId)
  }, [selectedWorkerId, loadSnapshot])

  useEffect(() => {
    if (!authToken || !selectedWorkerId) return

    let cancelled = false
    let cursor = Math.max((selectedWorkerSummary?.latest_seq ?? 0) - 1, 0)

    const run = async () => {
      while (!cancelled) {
        try {
          const { events, nextCursor } = await streamRuntimeCodexWorker(
            authToken,
            selectedWorkerId,
            cursor,
          )
          cursor = nextCursor
          setStreamState("live")
          setStreamError(null)

          if (events.length > 0) {
            setStreamEvents((current) => {
              const merged = [...events.reverse(), ...current]
              return merged.slice(0, STREAM_LIMIT)
            })
            void loadSnapshot(selectedWorkerId, true)
            void loadWorkers(true)
          }
        } catch (error) {
          const message =
            error instanceof RuntimeCodexApiError ? error.message : "Worker stream disconnected."
          setStreamState("reconnecting")
          setStreamError(message)

          if (error instanceof RuntimeCodexApiError && error.code === "auth") {
            return
          }

          await new Promise((resolve) => setTimeout(resolve, STREAM_RETRY_MS))
        }
      }
    }

    void run()

    return () => {
      cancelled = true
    }
  }, [authToken, selectedWorkerId, selectedWorkerSummary?.latest_seq, loadSnapshot, loadWorkers])

  const canAdmin = !!authToken && !adminDenied

  const handleRequest = useCallback(async () => {
    if (!authToken || !selectedWorkerId) return
    setRequestBusy(true)
    setLastAction(null)

    try {
      const method = requestMethod.trim()
      if (method.length < 3) {
        throw new RuntimeCodexApiError("request method is required", "invalid")
      }

      let params: Record<string, unknown> = {}
      const rawParams = requestParams.trim()
      if (rawParams.length > 0) {
        const parsed = JSON.parse(rawParams)
        params =
          typeof parsed === "object" && parsed !== null ? (parsed as Record<string, unknown>) : {}
      }

      const response = await requestRuntimeCodexWorker(authToken, selectedWorkerId, method, params)
      setLastAction(`Request ${response.request_id} ${response.ok ? "ok" : "failed"}.`)
      void loadSnapshot(selectedWorkerId, true)
      void loadWorkers(true)
    } catch (error) {
      if (error instanceof RuntimeCodexApiError && error.code === "forbidden") {
        setAdminDenied(true)
      }
      setLastAction(
        error instanceof RuntimeCodexApiError ? error.message : "Failed to send worker request.",
      )
    } finally {
      setRequestBusy(false)
    }
  }, [authToken, selectedWorkerId, requestMethod, requestParams, loadSnapshot, loadWorkers])

  const handleStop = useCallback(async () => {
    if (!authToken || !selectedWorkerId) return
    setStopBusy(true)
    setLastAction(null)

    try {
      const response = await stopRuntimeCodexWorker(authToken, selectedWorkerId)
      setLastAction(
        `Worker ${response.worker_id} stop ${response.idempotent_replay ? "replayed" : "accepted"}.`,
      )
      void loadSnapshot(selectedWorkerId, true)
      void loadWorkers(true)
    } catch (error) {
      if (error instanceof RuntimeCodexApiError && error.code === "forbidden") {
        setAdminDenied(true)
      }
      setLastAction(
        error instanceof RuntimeCodexApiError ? error.message : "Failed to stop worker.",
      )
    } finally {
      setStopBusy(false)
    }
  }, [authToken, selectedWorkerId, loadSnapshot, loadWorkers])

  if (!isAuthenticated) {
    return (
      <Screen preset="fixed" safeAreaEdges={["top"]} contentContainerStyle={themed($container)}>
        <Text text="Sign in to view Codex workers." />
      </Screen>
    )
  }

  if (!authToken) {
    return (
      <Screen preset="fixed" safeAreaEdges={["top"]} contentContainerStyle={themed($container)}>
        <Text text="Log out and sign in again to load runtime and Convex data." />
      </Screen>
    )
  }

  return (
    <Screen preset="scroll" safeAreaEdges={["top"]} contentContainerStyle={themed($container)}>
      <Text text="Codex Workers" preset="heading" style={themed($title)} />
      <Text
        text="Runtime APIs are authoritative; convex projection badges come from worker summaries."
        size="sm"
        style={themed($subtitle)}
      />

      <View style={themed($row)}>
        <Button
          text={workersLoading ? "Refreshing…" : "Refresh"}
          onPress={() => void loadWorkers()}
          style={themed($rowButton)}
          disabled={workersLoading}
        />
        <Button
          text={`Filter: ${filter}`}
          onPress={() => {
            setFilter((current) => {
              const next: WorkerFilter[] = ["all", "running", "stopped", "failed"]
              const idx = next.indexOf(current)
              return next[(idx + 1) % next.length]
            })
          }}
          style={themed($rowButton)}
        />
      </View>

      {workersError ? <Text text={workersError} size="sm" style={themed($errorText)} /> : null}

      <View style={themed($card)}>
        <Text text="Workers" preset="bold" />
        {workers.length === 0 ? (
          <Text text="No workers for this account/filter." size="sm" style={themed($muted)} />
        ) : (
          <FlatList
            data={workers}
            keyExtractor={(item) => item.worker_id}
            renderItem={({ item }) => {
              const selected = item.worker_id === selectedWorkerId
              return (
                <View style={themed([$workerRow, selected && $workerRowSelected])}>
                  <Button
                    text={item.worker_id}
                    onPress={() => setSelectedWorkerId(item.worker_id)}
                    style={themed($workerButton)}
                  />
                  <Text
                    text={`status ${item.status} · seq ${item.latest_seq} · hb ${item.heartbeat_state}`}
                    size="xs"
                    style={themed($muted)}
                  />
                  {item.convex_projection ? (
                    <Text
                      text={`convex ${item.convex_projection.status} · lag ${item.convex_projection.lag_events}`}
                      size="xs"
                      style={themed($muted)}
                    />
                  ) : (
                    <Text text="convex pending" size="xs" style={themed($muted)} />
                  )}
                </View>
              )
            }}
          />
        )}
      </View>

      <View style={themed($card)}>
        <Text text="Snapshot" preset="bold" />
        {!selectedWorkerId ? (
          <Text text="Select a worker to view snapshot." size="sm" style={themed($muted)} />
        ) : snapshotLoading ? (
          <Text text="Loading snapshot…" size="sm" style={themed($muted)} />
        ) : snapshotError ? (
          <Text text={snapshotError} size="sm" style={themed($errorText)} />
        ) : snapshot ? (
          <>
            <Text text={`worker ${snapshot.worker_id}`} size="sm" />
            <Text text={`status ${snapshot.status} · seq ${snapshot.latest_seq}`} size="sm" />
            <Text
              text={`heartbeat ${snapshot.heartbeat_state} · age ${snapshot.heartbeat_age_ms ?? "n/a"} ms`}
              size="sm"
              style={themed($muted)}
            />
            <Text
              text={`workspace ${snapshot.workspace_ref ?? "n/a"}`}
              size="xs"
              style={themed($muted)}
            />
          </>
        ) : (
          <Text text="Snapshot unavailable." size="sm" style={themed($muted)} />
        )}
      </View>

      <View style={themed($card)}>
        <Text text="Admin actions" preset="bold" />
        {adminDenied ? (
          <Text
            text="Admin controls disabled for this account (runtime policy returned forbidden)."
            size="sm"
            style={themed($errorText)}
          />
        ) : null}
        <TextField
          label="Request method"
          value={requestMethod}
          onChangeText={setRequestMethod}
          autoCapitalize="none"
          autoCorrect={false}
          editable={canAdmin}
          containerStyle={themed($field)}
        />
        <TextField
          label="Request params (JSON)"
          value={requestParams}
          onChangeText={setRequestParams}
          autoCapitalize="none"
          autoCorrect={false}
          editable={canAdmin}
          multiline
          containerStyle={themed($field)}
        />
        <View style={themed($row)}>
          <Button
            text={requestBusy ? "Sending…" : "Send request"}
            onPress={() => void handleRequest()}
            disabled={!canAdmin || !selectedWorkerId || requestBusy}
            style={themed($rowButton)}
          />
          <Button
            text={stopBusy ? "Stopping…" : "Stop worker"}
            onPress={() => void handleStop()}
            disabled={!canAdmin || !selectedWorkerId || stopBusy}
            style={themed($rowButton)}
          />
        </View>
        {lastAction ? <Text text={lastAction} size="sm" style={themed($muted)} /> : null}
      </View>

      <View style={themed($card)}>
        <Text text="Worker stream" preset="bold" />
        <Text text={`state ${streamState}`} size="xs" style={themed($muted)} />
        {streamError ? <Text text={streamError} size="sm" style={themed($errorText)} /> : null}
        {streamEvents.length === 0 ? (
          <Text text="No stream events yet." size="sm" style={themed($muted)} />
        ) : (
          streamEvents.slice(0, 25).map((event, index) => (
            <View key={`${event.id ?? "none"}:${index}`} style={themed($eventRow)}>
              <Text text={`#${event.id ?? "?"} ${event.event}`} size="xs" />
              <Text
                text={JSON.stringify(event.payload).slice(0, 180)}
                size="xs"
                style={themed($muted)}
              />
            </View>
          ))
        )}
      </View>
    </Screen>
  )
}

const $container: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.lg,
  paddingTop: spacing.lg,
  paddingBottom: spacing.xxl,
  gap: spacing.md,
})

const $title: ThemedStyle<TextStyle> = ({ spacing }) => ({
  marginBottom: spacing.xs,
})

const $subtitle: ThemedStyle<TextStyle> = ({ colors }) => ({
  color: colors.textDim,
})

const $card: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  backgroundColor: colors.palette.neutral100,
  borderRadius: 12,
  padding: spacing.md,
  gap: spacing.sm,
})

const $row: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  gap: spacing.sm,
})

const $rowButton: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flex: 1,
  minHeight: 44,
  marginTop: spacing.xs,
})

const $field: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  marginTop: spacing.xs,
})

const $muted: ThemedStyle<TextStyle> = ({ colors }) => ({
  color: colors.textDim,
})

const $errorText: ThemedStyle<TextStyle> = ({ colors }) => ({
  color: colors.error,
})

const $workerRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  borderRadius: 8,
  paddingVertical: spacing.xs,
  gap: spacing.xs,
})

const $workerRowSelected: ThemedStyle<ViewStyle> = ({ colors }) => ({
  backgroundColor: colors.palette.neutral200,
})

const $workerButton: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  minHeight: 40,
  marginTop: spacing.xs,
})

const $eventRow: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  borderTopWidth: 1,
  borderColor: colors.separator,
  paddingTop: spacing.xs,
})
