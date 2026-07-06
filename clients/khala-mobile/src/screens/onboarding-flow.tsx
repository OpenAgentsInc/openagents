import { useEffect, useState } from "react"
import { ScrollView, View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { CreditsBalanceChip } from "../components/credits-balance-chip"
import { KhalaButton } from "../components/khala-button"
import { KhalaEmptyState } from "../components/khala-empty-state"
import { KhalaListItem } from "../components/khala-list-item"
import { KhalaText } from "../components/khala-text"
import { KhalaTextField } from "../components/khala-text-field"
import { registerForPushNotificationsAsync } from "../push/push-notifications-client"
import { fetchKhalaMobileCreditsBalance } from "../sync/khala-mobile-credits-api"
import { formatUsdCents } from "../sync/khala-mobile-credits-format-core"
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

const WelcomeStep = ({ onContinue }: { onContinue: () => void }) => (
  <ScrollView className="flex-1" contentContainerClassName="flex-1 justify-center gap-6 px-6 py-8">
    <View className="gap-2">
      <KhalaText className="text-center" variant="heading">
        Welcome to Khala Code
      </KhalaText>
      <KhalaText className="text-center" variant="muted">
        Pick a repo, ask the agent to do something, and watch it work — right
        from your phone.
      </KhalaText>
    </View>
    <View className="items-center">
      <CreditsBalanceChip />
    </View>
    <KhalaButton onPress={onContinue} text="Get started" variant="primary" />
  </ScrollView>
)

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
    <View className="flex-1 gap-4 px-6 py-8">
      <View className="gap-1">
        <KhalaText variant="heading">Pick a repo</KhalaText>
        <KhalaText variant="muted">You can change this later, or skip for a repo-less chat.</KhalaText>
      </View>
      <KhalaTextField
        autoCapitalize="none"
        autoCorrect={false}
        label="Search"
        mono={false}
        onChangeText={setSearchTerm}
        placeholder="owner/repo"
        value={searchTerm}
      />
      {state.status === "loading" ? (
        <KhalaEmptyState loading title="Loading your repositories" tone="accent" />
      ) : state.status === "error" ? (
        <KhalaEmptyState detail={state.messageSafe} title="Repositories unavailable" tone="danger" />
      ) : visibleRepos.length === 0 ? (
        <KhalaEmptyState title={searchTerm.trim().length === 0 ? "No repositories found" : "No matching repositories"} />
      ) : (
        <ScrollView className="flex-1">
          {visibleRepos.slice(0, 50).map(repo => (
            <KhalaListItem
              accessibilityLabel={repo.fullName}
              detail={repo.description ?? undefined}
              key={repo.id}
              meta={repo.private ? "private" : "public"}
              onPress={() => onSelect({ defaultBranch: repo.defaultBranch, name: repo.name, owner: repo.owner })}
              title={repo.fullName}
            />
          ))}
        </ScrollView>
      )}
      <View className="gap-2">
        <KhalaButton onPress={onSkip} text="Skip — start without a repo" variant="secondary" />
        <KhalaButton onPress={onBack} text="Back" variant="ghost" />
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
    <ScrollView className="flex-1" contentContainerClassName="gap-4 px-6 py-8">
      <View className="gap-1">
        <KhalaText variant="heading">What should Khala Code do?</KhalaText>
        <KhalaText variant="muted">
          {selectedRepo === null
            ? "No repo selected — this will be a plain chat."
            : `Working in ${selectedRepo.owner}/${selectedRepo.name}.`}
        </KhalaText>
      </View>

      <View className="gap-2">
        {ONBOARDING_SUGGESTED_TASKS.map(task => (
          <KhalaButton
            disabled={creating}
            key={task.id}
            onPress={() => setTaskText(task.prompt)}
            text={task.label}
            variant={taskText === task.prompt ? "primary" : "secondary"}
          />
        ))}
      </View>

      <KhalaTextField
        label="Or describe your own task"
        mono={false}
        multiline
        onChangeText={setTaskText}
        placeholder="What do you want done?"
        value={taskText}
      />

      {zeroBalanceBlock ? (
        <KhalaText variant="danger">You're out of credits. Add more in Settings to start a task.</KhalaText>
      ) : null}
      {errorMessage === null ? null : <KhalaText variant="danger">{errorMessage}</KhalaText>}

      <KhalaButton
        disabled={taskText.trim().length === 0 || creating || zeroBalanceBlock}
        loading={creating}
        onPress={() => void startTask(taskText)}
        text="Start"
        variant="primary"
      />
      <KhalaButton disabled={creating} onPress={onBack} text="Back" variant="ghost" />
    </ScrollView>
  )
}
