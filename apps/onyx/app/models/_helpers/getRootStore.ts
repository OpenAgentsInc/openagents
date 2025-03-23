import { createContext, useContext } from "react"
import { RootStore } from "../RootStore"

/**
 * Create the initial context
 */
export const RootStoreContext = createContext<RootStore>({} as RootStore)

/**
 * The provider our root component will use to expose the root store
 */
export const RootStoreProvider = RootStoreContext.Provider

/**
 * A hook that screens can use to gain access to our stores:
 *
 * const rootStore = useStores()
 */
export const useStores = () => useContext(RootStoreContext)