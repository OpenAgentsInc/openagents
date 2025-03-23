import { createContext, useContext, useEffect, useState } from "react"
import { log } from "@/utils/log"
import {
  createRootStoreDefaultModel, RootStore, RootStoreModel
} from "../RootStore"
import { setupRootStore } from "./setupRootStore"
import Config from "../../config"

/**
 * Create the initial (empty) global RootStore instance here.
 */
const _rootStore = createRootStoreDefaultModel()

/**
 * The RootStoreContext provides a way to access
 * the RootStore in any screen or component.
 */
export const RootStoreContext = createContext<RootStore>(_rootStore)

/**
 * The provider our root component will use to expose the root store
 */
export const RootStoreProvider = RootStoreContext.Provider

/**
 * A hook that screens and other components can use to gain access to our stores
 */
export const useStores = () => useContext(RootStoreContext)

/**
 * Used only in the app.tsx file, this hook sets up the RootStore
 * and then rehydrates it.
 */
export const useInitialRootStore = (callback?: () => void | Promise<void>) => {
  const rootStore = useStores()
  const [rehydrated, setRehydrated] = useState(false)
  const [config, setConfig] = useState(Config)

  // Kick off initial async loading actions, like loading fonts and rehydrating RootStore
  useEffect(() => {
    let _unsubscribe: () => void | undefined
      ; (async () => {
        try {
          // set up the RootStore (returns the state restored from AsyncStorage)
          const { unsubscribe } = await setupRootStore(rootStore)
          _unsubscribe = unsubscribe

          // reactotron integration with the MST root store (DEV only)
          if (__DEV__) {
            // @ts-ignore
            console.tron?.trackMstNode(rootStore)
          }

          await rootStore.walletStore.setup()
          await rootStore.walletStore.fetchBalanceInfo()
          await rootStore.walletStore.fetchTransactions()

          // let the app know we've finished rehydrating
          setRehydrated(true)

          // invoke the callback, if provided
          if (callback) await callback()
        } catch (error) {
          console.error("Failed to setup root store:", error)
          // Still set rehydrated to true to prevent infinite loading
          setRehydrated(true)
        }
      })()

    return () => {
      // cleanup
      if (_unsubscribe !== undefined) _unsubscribe()
    }
  }, []) // Empty dependency array since we only want this to run once on mount

  return { rehydrated, config }
}