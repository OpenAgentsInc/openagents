import { useEffect, useState } from "react"
import { FlatList, View, type TextStyle, type ViewStyle } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { EmptyState, Header, ListItem, Screen, Text, TextField, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import type { AppStackScreenProps } from "../navigators/navigationTypes"
import { useKhalaMobileSyncRuntime } from "../sync/khala-mobile-sync-runtime-context"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../theme/motion"
import {
  dedupeKhalaMobileRepositoriesById,
  filterKhalaMobileRepositories,
  sortKhalaMobileRepositoriesForPicker,
} from "../sync/khala-mobile-repo-search-core"
import { fetchKhalaMobileRepositories, type KhalaMobileRepository } from "../sync/khala-mobile-repos-api"

type RepoPickerScreenProps = AppStackScreenProps<"RepoPicker">

const REPOS_PER_PAGE = 100

// Matches thread-messages-screen.tsx / thread-list-screen.tsx's stagger —
// arcade-fidelity audit (2026-07-06) §4.
const REPO_STAGGER_CAP = 8
const repoEntranceDelay = (index: number): number =>
  MOTION_STAGGER_MS * Math.min(index, REPO_STAGGER_CAP)

type LoadState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "error"; messageSafe: string }>
  | Readonly<{ status: "ready"; hasNextPage: boolean; page: number; repositories: ReadonlyArray<KhalaMobileRepository> }>

const RepoRow = ({
  binding,
  onPress,
  repo,
}: {
  binding: "pending" | null
  onPress: () => void
  repo: KhalaMobileRepository
}) => {
  const { themed } = useAppTheme()
  const visibilityLabel = repo.private ? "private" : "public"
  return (
    <ListItem
      // Announce visibility in the accessible label: a ListItem with an
      // accessibilityLabel collapses its children, so the "public"/"private"
      // RightComponent text would otherwise never reach the accessibility tree
      // (screen readers or the mobile visual harness). Keeping it here makes the
      // visibility a first-class, testable part of each real repo row.
      accessibilityLabel={`${repo.fullName}, ${visibilityLabel}`}
      disabled={binding === "pending"}
      onPress={onPress}
      TextProps={{ weight: "medium", size: "sm" }}
      RightComponent={<Text size="xs" style={themed($meta)} text={visibilityLabel} />}
    >
      {repo.fullName}
      {repo.description ? "\n" : ""}
      {repo.description ? <Text size="xs" style={themed($dim)} text={repo.description} /> : null}
    </ListItem>
  )
}

/**
 * MM-I3 (#8492): the "pick a repo" step of the mobile-only MVP straight line,
 * rebuilt on the ported Infinite Red Ignite component kit (`../ignite`) for the
 * real Ignite look. Behavior — load/search/select/error — is unchanged.
 */
export const RepoPickerScreen = ({ navigation, route }: RepoPickerScreenProps) => {
  const { threadId } = route.params
  const { baseUrl, token } = useKhalaAuth()
  const { themed } = useAppTheme()
  const runtimeState = useKhalaMobileSyncRuntime()
  const [state, setState] = useState<LoadState>({ status: "loading" })
  const [searchTerm, setSearchTerm] = useState("")
  const [bindingRepoId, setBindingRepoId] = useState<string | null>(null)
  const [bindError, setBindError] = useState<string | null>(null)

  const loadPage = async (page: number, append: boolean) => {
    if (token === "") {
      setState({ messageSafe: "Sign in again to see your repositories.", status: "error" })
      return
    }
    const result = await fetchKhalaMobileRepositories(baseUrl, token, { page, perPage: REPOS_PER_PAGE })
    if (!result.ok) {
      setState({ messageSafe: result.messageSafe, status: "error" })
      return
    }
    setState(previous => {
      const priorRepos = append && previous.status === "ready" ? previous.repositories : []
      return {
        hasNextPage: result.value.hasNextPage,
        page,
        repositories: dedupeKhalaMobileRepositoriesById([...priorRepos, ...result.value.repositories]),
        status: "ready",
      }
    })
  }

  useEffect(() => {
    void loadPage(1, false)
    // Intentionally runs once per screen mount; pagination continues via
    // explicit "Load more" taps, not by re-running this on token/baseUrl
    // refresh mid-session.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const handleSelectRepo = async (repo: KhalaMobileRepository) => {
    if (bindingRepoId !== null || runtimeState.status !== "ready") return
    setBindingRepoId(repo.id)
    setBindError(null)
    const result = await runtimeState.runtime.bindThreadRepo({
      repo: { defaultBranch: repo.defaultBranch, name: repo.name, owner: repo.owner },
      threadId,
    })
    setBindingRepoId(null)
    if (!result.ok) {
      setBindError(result.error ?? "Could not bind this repo to the thread.")
      return
    }
    if (navigation.canGoBack()) navigation.goBack()
  }

  const visibleRepos =
    state.status === "ready"
      ? filterKhalaMobileRepositories(sortKhalaMobileRepositoriesForPicker(state.repositories), searchTerm)
      : []

  return (
    <Screen preset="fixed" contentContainerStyle={themed($fill)}>
      <Header
        title="Pick a repo"
        leftIcon="‹"
        onLeftPress={() => {
          if (navigation.canGoBack()) navigation.goBack()
        }}
      />
      <View style={themed($searchRow)}>
        <TextField
          autoCapitalize="none"
          autoCorrect={false}
          label="Search"
          onChangeText={setSearchTerm}
          placeholder="owner/repo"
          value={searchTerm}
        />
      </View>
      {bindError === null ? null : (
        <View style={themed($errorRow)}>
          <Text size="xs" style={themed($danger)} text={bindError} />
        </View>
      )}
      {state.status === "loading" ? (
        <View style={themed($centered)}>
          <EmptyState loading heading="Loading repositories" />
        </View>
      ) : state.status === "error" ? (
        <View style={themed($centered)}>
          <EmptyState status="error" heading="Repositories unavailable" content={state.messageSafe} />
        </View>
      ) : visibleRepos.length === 0 ? (
        <View style={themed($centered)}>
          <EmptyState heading={searchTerm.trim().length === 0 ? "No repositories found" : "No matching repositories"} />
        </View>
      ) : (
        <FlatList
          ItemSeparatorComponent={() => <View style={themed($separator)} />}
          ListFooterComponent={
            state.hasNextPage && searchTerm.trim().length === 0 ? (
              <ListItem
                accessibilityLabel="Load more repositories"
                text="Load more"
                onPress={() => void loadPage(state.page + 1, true)}
              />
            ) : (
              <View style={themed($footerSpacer)} />
            )
          }
          data={visibleRepos}
          keyExtractor={repo => repo.id}
          renderItem={({ index, item: repo }) => (
            <Animated.View entering={FadeIn.delay(repoEntranceDelay(index)).duration(MOTION_MEDIUM)}>
              <RepoRow
                binding={bindingRepoId === repo.id ? "pending" : null}
                onPress={() => void handleSelectRepo(repo)}
                repo={repo}
              />
            </Animated.View>
          )}
        />
      )}
    </Screen>
  )
}

const $fill: ThemedStyle<ViewStyle> = () => ({ flex: 1 })

const $searchRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.md,
  paddingTop: spacing.sm,
  paddingBottom: spacing.xs,
})

const $errorRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.md,
  paddingBottom: spacing.xs,
})

const $centered: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flex: 1,
  justifyContent: "center",
  paddingHorizontal: spacing.md,
})

const $separator: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  height: 1,
  marginHorizontal: spacing.md,
  backgroundColor: colors.separator,
})

const $footerSpacer: ThemedStyle<ViewStyle> = ({ spacing }) => ({ height: spacing.xl })

const $dim: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })
const $meta: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim, paddingTop: 2 })
const $danger: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.error })
