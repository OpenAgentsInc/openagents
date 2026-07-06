import { useEffect, useState } from "react"
import { FlatList, View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { AppHeader } from "../components/app-header"
import { KhalaEmptyState } from "../components/khala-empty-state"
import { KhalaListItem } from "../components/khala-list-item"
import { KhalaScreen } from "../components/khala-screen"
import { KhalaText } from "../components/khala-text"
import { KhalaTextField } from "../components/khala-text-field"
import type { AppStackScreenProps } from "../navigators/navigationTypes"
import { useKhalaMobileSyncRuntime } from "../sync/khala-mobile-sync-runtime-context"
import {
  dedupeKhalaMobileRepositoriesById,
  filterKhalaMobileRepositories,
  sortKhalaMobileRepositoriesForPicker,
} from "../sync/khala-mobile-repo-search-core"
import { fetchKhalaMobileRepositories, type KhalaMobileRepository } from "../sync/khala-mobile-repos-api"

type RepoPickerScreenProps = AppStackScreenProps<"RepoPicker">

const REPOS_PER_PAGE = 100

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
}) => (
  <KhalaListItem
    accessibilityLabel={repo.fullName}
    detail={repo.description ?? undefined}
    disabled={binding === "pending"}
    meta={repo.private ? "private" : "public"}
    onPress={onPress}
    title={repo.fullName}
  />
)

export const RepoPickerScreen = ({ navigation, route }: RepoPickerScreenProps) => {
  const { threadId } = route.params
  const { baseUrl, token } = useKhalaAuth()
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
    <KhalaScreen preset="fixed">
      <AppHeader showBack title="Pick a repo" />
      <View className="px-4 pb-2 pt-3">
        <KhalaTextField
          autoCapitalize="none"
          autoCorrect={false}
          label="Search"
          mono={false}
          onChangeText={setSearchTerm}
          placeholder="owner/repo"
          value={searchTerm}
        />
      </View>
      {bindError === null ? null : (
        <View className="px-4 pb-2">
          <KhalaText variant="danger">{bindError}</KhalaText>
        </View>
      )}
      {state.status === "loading" ? (
        <View className="flex-1 justify-center px-4">
          <KhalaEmptyState loading title="Loading repositories" tone="accent" />
        </View>
      ) : state.status === "error" ? (
        <View className="flex-1 justify-center px-4">
          <KhalaEmptyState detail={state.messageSafe} title="Repositories unavailable" tone="danger" />
        </View>
      ) : visibleRepos.length === 0 ? (
        <View className="flex-1 justify-center px-4">
          <KhalaEmptyState
            title={searchTerm.trim().length === 0 ? "No repositories found" : "No matching repositories"}
          />
        </View>
      ) : (
        <FlatList
          ItemSeparatorComponent={() => <View className="mx-4 h-px bg-borderMuted" />}
          ListFooterComponent={
            state.hasNextPage && searchTerm.trim().length === 0 ? (
              <KhalaListItem
                accessibilityLabel="Load more repositories"
                onPress={() => void loadPage(state.page + 1, true)}
                title="Load more"
              />
            ) : (
              <View className="h-8" />
            )
          }
          data={visibleRepos}
          keyExtractor={repo => repo.id}
          renderItem={({ item: repo }) => (
            <RepoRow
              binding={bindingRepoId === repo.id ? "pending" : null}
              onPress={() => void handleSelectRepo(repo)}
              repo={repo}
            />
          )}
        />
      )}
    </KhalaScreen>
  )
}
