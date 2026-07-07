const warning = "rgb(250, 186, 48)"
const error = "rgb(255, 85, 105)"

export const getBackgroundColor = (opacity) => `rgba(15, 23, 34, ${opacity})`
export const getBackgroundLightColor = (opacity) => `rgba(24, 35, 50, ${opacity})`
export const getBackgroundDarkColor = (opacity) => `rgba(8, 13, 20, ${opacity})`
export const getTextColor = (opacity) => `rgba(248, 253, 255, ${opacity})`
export const getWarningColor = (opacity) => `rgba(250, 186, 48, ${opacity})`
export const getWarningDarkColor = (opacity) => `rgba(224, 167, 8, ${opacity})`
export const getErrorColor = (opacity) => `rgba(255, 85, 105, ${opacity})`
export const getErrorDarkColor = (opacity) => `rgba(208, 75, 95, ${opacity})`
export const getFatalColor = (opacity) => `rgba(255, 85, 105, ${opacity})`
export const getFatalDarkColor = (opacity) => `rgba(208, 75, 95, ${opacity})`
export const getLogColor = (opacity) => `rgba(186, 199, 213, ${opacity})`
export const getWarningHighlightColor = (opacity) => `rgba(252, 176, 29, ${opacity})`
export const getDividerColor = (opacity) => `rgba(248, 253, 255, ${opacity})`
export const getHighlightColor = (opacity) => `rgba(252, 176, 29, ${opacity})`

export const WarningIconColor = warning
export const ErrorIconColor = error
