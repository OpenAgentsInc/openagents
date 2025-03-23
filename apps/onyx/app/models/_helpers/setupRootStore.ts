import { applySnapshot, onSnapshot } from "mobx-state-tree"
import * as storage from "../../utils/storage"
import { RootStore, RootStoreSnapshotIn } from "../RootStore"

/**
 * The key we'll be saving our state as within async storage.
 */
const ROOT_STATE_STORAGE_KEY = "root-v1aaaf341x167"

/**
 * Setup the root state.
 */
export async function setupRootStore(rootStore: RootStore) {
  let restoredState: RootStoreSnapshotIn | undefined | null

  try {
    // load the last known state from AsyncStorage
    restoredState = ((await storage.load(ROOT_STATE_STORAGE_KEY)) ?? {}) as RootStoreSnapshotIn
    applySnapshot(rootStore, restoredState)
  } catch (e) {
    // if there's any problems loading, then inform the dev what happened
    if (__DEV__) {
      console.error(e instanceof Error ? e.message : "Error loading root store")
    }
  }

  // track changes & save to AsyncStorage
  const unsubscribe = onSnapshot(rootStore, (snapshot) => storage.save(ROOT_STATE_STORAGE_KEY, snapshot))

  return { rootStore, restoredState, unsubscribe }
}
