import { useEffect, useState } from "react"
import { ScrollView, View, type TextStyle, type ViewStyle } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { CreditsBalanceChip } from "../components/credits-balance-chip"
import { Button, EmptyState, ListItem, Text, TextField, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import { registerForPushNotificationsAsync } from "../push/push-notifications-client"
import { fetchKhalaMobileCreditsBalance } from "../sync/khala-mobile-credits-api"
import {
  dedupeKhalaMobileRepositoriesById,
  filterKhalaMobileRepositories,
  sortKhalaMobileRepositoriesForPicker,
} from "../sync/khala-mobile-repo-search-core"
import { fetchKhalaMobileRepositories, type KhalaMobileRepository } from "../sync/khala-mobile-repos-api"
import { useKhalaMobileSyncRuntime } from "../sync/khala-mobile-sync-runtime-context"
import { buildChatAppendMessageArgs, buildStartTurnIntentArgs, chatMessageBodyRef, DEFAULT_RUNTIME_LANE } from "../sync/khala-runtime-compose-core"
import { makeSafeRef } from "../sync/khala-sync-push-core"
import { useKhalaSyncPush } from "../sync/use-khala-sync-push"
import { blocksOnZeroBalance, deriveThreadTitleFromTask, ONBOARDING_SUGGESTED_TASKS, type OnboardingRepoBinding } from "./onboarding-core"

type OnboardingStep = "welcome" | "repo" | "task"

export type OnboardingFlowProps = Readonly<{
  onThreadCreated: (input: { threadId: string; title: string }) => void
}>

/**
 * MM-H2 (#8488): the mobile-only MVP's first-run straight line — sign in
 * (already done by the time this mounts) → land with the $10 grant visible
 * → guided repo pick → a suggested first task → watch the turn stream.
 * Rendered by `thread-list-screen.tsx` in place of the empty-thread-list
 * state (see that file), so there is no separate onboarding navigation
 * route to get stuck on — the SAME screen a returning user's thread list
 * lives on is where a brand-new user starts.
 *
 * Rebuilt on the ported Infinite Red Ignite component kit (`../ignite`) so the
 * onboarding straight line shows the real Ignite look; product behavior and
 * copy are unchanged.
 */
export const OnboardingFlow = ({ onThreadCreated }: OnboardingFlowProps) => {
  const [step, setStep] = useState<OnboardingStep>("welcome")
  const [selectedRepo, setSelectedRepo] = useState<OnboardingRepoBinding | null>(null)

  if (step === "welcome") return <WelcomeStep onContinue={() => setStep("repo")} />
  if (step === "repo") {
    return (
      <RepoStep
        onBack={() => setStep("welcome")}
        onSelect={repo => {
          setSelectedRepo(repo)
          setStep("task")
        }}
        onSkip={() => {
          setSelectedRepo(null)
          setStep("task")
        }}
      />
    )
  }
  return (
    <TaskStep
      onBack={() => setStep("repo")}
      onThreadCreated={onThreadCreated}
      selectedRepo={selectedRepo}
    />
  )
}

const WelcomeStep = ({ onContinue }: { onContinue: () => void }) => {
  const { themed } = useAppTheme()
  return (
    <ScrollView style={themed($flex)} contentContainerStyle={themed($welcomeContent)}>
      <View style={themed($gapSm)}>
        <Text preset="heading" style={themed($center)} text="Welcome to Khala Code" />
        <Text
          style={themed($centerDim)}
          text="Pick a repo, ask the agent to do something, and watch it work — right from your phone."
        />
      </View>
      <View style={themed($centerRow)}>
        <CreditsBalanceChip />
      </View>
      <Button preset="reversed" onPress={onContinue} text="Get started" />
    </ScrollView>
  )
}

const REPO_STEP_PER_PAGE = 100

type RepoStepLoadState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "error"; messageSafe: string }>
  | Readonly<{ status: "ready"; repositories: ReadonlyArray<KhalaMobileRepository> }>

const RepoStep = ({
  onBack,
  onSelect,
  onSkip,
}: {
  onBack: () => void
  onSelect: (repo: OnboardingRepoBinding) => void
  onSkip: () => void
}) => {
  const { baseUrl, token } = useKhalaAuth()
  const { themed } = useAppTheme()
  const [state, setState] = useState<RepoStepLoadState>({ status: "loading" })
  const [searchTerm, setSearchTerm] = useState("")

  useEffect(() => {
    let cancelled = false
    void fetchKhalaMobileRepositories(baseUrl, token, { page: 1, perPage: REPO_STEP_PER_PAGE }).then(result => {
      if (cancelled) return
      setState(
        result.ok
          ? { repositories: dedupeKhalaMobileRepositoriesById(result.value.repositories), status: "ready" }
          : { messageSafe: result.messageSafe, status: "error" },
      )
    })
    return () => {
      cancelled = true
    }
  }, [baseUrl, token])

  const visibleRepos =
    state.status === "ready"
      ? filterKhalaMobileRepositories(sortKhalaMobileRepositoriesForPicker(state.repositories), searchTerm)
      : []

  return (
    <View style={themed($stepContainer)}>
      <View style={themed($gapXs)}>
        <Text preset="subheading" text="Pick a repo" />
        <Text style={themed($dim)} text="You can change this later, or skip for a repo-less chat." />
      </View>
      <TextField
        autoCapitalize="none"
        autoCorrect={false}
        label="Search"
        onChangeText={setSearchTerm}
        placeholder="owner/repo"
        value={searchTerm}
      />
      {state.status === "loading" ? (
        <EmptyState loading heading="Loading your repositories" />
      ) : state.status === "error" ? (
        <EmptyState status="error" heading="Repositories unavailable" content={state.messageSafe} />
      ) : visibleRepos.length === 0 ? (
        <EmptyState heading={searchTerm.trim().length === 0 ? "No repositories found" : "No matching repositories"} />
      ) : (
        <ScrollView style={themed($flex)}>
          {visibleRepos.slice(0, 50).map(repo => (
            <ListItem
              accessibilityLabel={repo.fullName}
              key={repo.id}
              onPress={() => onSelect({ defaultBranch: repo.defaultBranch, name: repo.name, owner: repo.owner })}
              TextProps={{ weight: "medium", size: "sm" }}
              RightComponent={<Text size="xs" style={themed($meta)} text={repo.private ? "private" : "public"} />}
            >
              {repo.fullName}
              {repo.description ? "\n" : ""}
              {repo.description ? <Text size="xs" style={themed($dim)} text={repo.description} /> : null}
            </ListItem>
          ))}
        </ScrollView>
      )}
      <View style={themed($gapXs)}>
        <Button preset="filled" onPress={onSkip} text="Skip — start without a repo" />
        <Button preset="default" onPress={onBack} text="Back" />
      </View>
    </View>
  )
}

const TaskStep = ({
  onBack,
  onThreadCreated,
  selectedRepo,
}: {
  onBack: () => void
  onThreadCreated: OnboardingFlowProps["onThreadCreated"]
  selectedRepo: OnboardingRepoBinding | null
}) => {
  const { baseUrl, token } = useKhalaAuth()
  const { themed } = useAppTheme()
  const runtimeState = useKhalaMobileSyncRuntime()
  const push = useKhalaSyncPush()
  const [taskText, setTaskText] = useState("")
  const [creating, setCreating] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [zeroBalanceBlock, setZeroBalanceBlock] = useState(false)

  useEffect(() => {
    let cancelled = false
    void fetchKhalaMobileCreditsBalance(baseUrl, token).then(result => {
      if (cancelled) return
      setZeroBalanceBlock(blocksOnZeroBalance(result.ok ? { ok: true, value: result.value } : { ok: false }))
    })
    return () => {
      cancelled = true
    }
  }, [baseUrl, token])

  const startTask = async (text: string) => {
    const trimmed = text.trim()
    if (trimmed.length === 0 || creating || zeroBalanceBlock || runtimeState.status !== "ready") return
    setCreating(true)
    setErrorMessage(null)
    try {
      const threadId = makeSafeRef("thread")
      const title = deriveThreadTitleFromTask(trimmed)
      const createResult = await runtimeState.runtime.createThread({ threadId, title })
      if (!createResult.ok) {
        throw new Error(createResult.error ?? "Could not create the thread.")
      }
      if (selectedRepo !== null) {
        await runtimeState.runtime.bindThreadRepo({ repo: selectedRepo, threadId })
      }
      const nowIso = new Date().toISOString()
      const messageId = makeSafeRef("msg")
      const bodyRef = chatMessageBodyRef(messageId)
      const turnId = makeSafeRef("turn")
      await push([
        { args: buildChatAppendMessageArgs({ body: trimmed, messageId, threadId }), name: "chat.appendMessage" },
        {
          args: buildStartTurnIntentArgs({ bodyRef, nowIso, target: { lane: DEFAULT_RUNTIME_LANE }, threadId, turnId }),
          name: "runtime.startTurn",
        },
      ])
      void registerForPushNotificationsAsync({ apiBaseUrl: baseUrl, bearerToken: token, event: "task_dispatched" })
      onThreadCreated({ threadId, title })
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error))
      setCreating(false)
    }
  }

  return (
    <ScrollView style={themed($flex)} contentContainerStyle={themed($taskContent)}>
      <View style={themed($gapXs)}>
        <Text preset="subheading" text="What should Khala Code do?" />
        <Text
          style={themed($dim)}
          text={
            selectedRepo === null
              ? "No repo selected — this will be a plain chat."
              : `Working in ${selectedRepo.owner}/${selectedRepo.name}.`
          }
        />
      </View>

      <View style={themed($gapXs)}>
        {ONBOARDING_SUGGESTED_TASKS.map(task => (
          <Button
            disabled={creating}
            key={task.id}
            onPress={() => setTaskText(task.prompt)}
            preset={taskText === task.prompt ? "reversed" : "filled"}
            text={task.label}
          />
        ))}
      </View>

      <TextField
        label="Or describe your own task"
        multiline
        onChangeText={setTaskText}
        placeholder="What do you want done?"
        value={taskText}
      />

      {zeroBalanceBlock ? (
        <Text style={themed($danger)} text="You're out of credits. Add more in Settings to start a task." />
      ) : null}
      {errorMessage === null ? null : <Text style={themed($danger)} text={errorMessage} />}

      <Button
        disabled={taskText.trim().length === 0 || creating || zeroBalanceBlock}
        preset="reversed"
        onPress={() => void startTask(taskText)}
        text="Start"
      />
      <Button preset="default" disabled={creating} onPress={onBack} text="Back" />
    </ScrollView>
  )
}

const $flex: ThemedStyle<ViewStyle> = () => ({ flex: 1 })

const $welcomeContent: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexGrow: 1,
  justifyContent: "center",
  gap: spacing.lg,
  paddingHorizontal: spacing.lg,
  paddingVertical: spacing.xl,
})

const $taskContent: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  gap: spacing.md,
  paddingHorizontal: spacing.lg,
  paddingVertical: spacing.xl,
})

const $stepContainer: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flex: 1,
  gap: spacing.md,
  paddingHorizontal: spacing.lg,
  paddingVertical: spacing.xl,
})

const $gapSm: ThemedStyle<ViewStyle> = ({ spacing }) => ({ gap: spacing.xs })
const $gapXs: ThemedStyle<ViewStyle> = ({ spacing }) => ({ gap: spacing.xs })
const $centerRow: ThemedStyle<ViewStyle> = () => ({ alignItems: "center" })

const $center: ThemedStyle<TextStyle> = () => ({ textAlign: "center" })
const $centerDim: ThemedStyle<TextStyle> = ({ colors }) => ({ textAlign: "center", color: colors.textDim })
const $dim: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })
const $meta: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim, paddingTop: 2 })
const $danger: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.error })
