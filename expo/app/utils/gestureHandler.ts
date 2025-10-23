// Don't import react-native-gesture-handler on web
// https://reactnavigation.org/docs/drawer-navigator/#installation

// Polyfill needed for RNGH on web
// https://github.com/software-mansion/react-native-gesture-handler/issues/2402
import "setimmediate"

