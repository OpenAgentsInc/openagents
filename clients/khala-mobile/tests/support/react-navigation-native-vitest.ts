const state = { index: 0, routes: [] }

export const createNavigationContainerRef = () => ({
  canGoBack: () => false,
  getRootState: () => state,
  goBack() {},
  isReady: () => false,
  resetRoot() {},
})
export const useScrollToTop = () => undefined
