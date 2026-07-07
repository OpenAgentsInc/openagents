const warning = "rgb(250, 186, 48)"
const error = "rgb(255, 85, 105)"

export const getBackgroundColor = (opacity) => `rgba(248, 250, 252, ${opacity})`
export const getBackgroundLightColor = (opacity) => `rgba(226, 232, 240, ${opacity})`
export const getBackgroundDarkColor = (opacity) => `rgba(203, 213, 225, ${opacity})`
export const getTextColor = (opacity) => `rgba(10, 18, 28, ${opacity})`
export const getWarningColor = (opacity) => `rgba(250, 186, 48, ${opacity})`
export const getWarningDarkColor = (opacity) => `rgba(224, 167, 8, ${opacity})`
export const getErrorColor = (opacity) => `rgba(255, 85, 105, ${opacity})`
export const getErrorDarkColor = (opacity) => `rgba(208, 75, 95, ${opacity})`
export const getFatalColor = (opacity) => `rgba(255, 85, 105, ${opacity})`
export const getFatalDarkColor = (opacity) => `rgba(208, 75, 95, ${opacity})`
export const getLogColor = (opacity) => `rgba(71, 85, 105, ${opacity})`
export const getWarningHighlightColor = (opacity) => `rgba(252, 176, 29, ${opacity})`
export const getDividerColor = (opacity) => `rgba(10, 18, 28, ${opacity})`
export const getHighlightColor = (opacity) => `rgba(252, 176, 29, ${opacity})`

export const WarningIconColor = warning
export const ErrorIconColor = error
