export type DrawerButtonBars = {
  topBar: {
    translateX: number
    rotateDeg: number
    width: number
    colorStop: number
    marginBottom: number
  }
  middleBar: {
    width: number
    colorStop: number
  }
  bottomBar: {
    translateX: number
    rotateDeg: number
    width: number
    colorStop: number
    marginTop: number
  }
  container: {
    translateX: number
  }
}

const clampProgress = (progress: number): number => {
  if (Number.isNaN(progress)) return 0
  return Math.min(1, Math.max(0, progress))
}

const interpolate = (progress: number, from: number, to: number): number => {
  return from + (to - from) * progress
}

export const drawerButtonBars = (progress: number): DrawerButtonBars => {
  const clampedProgress = clampProgress(progress)

  return {
    topBar: {
      translateX: interpolate(clampedProgress, 0, -11.5),
      rotateDeg: interpolate(clampedProgress, 0, -45),
      width: interpolate(clampedProgress, 18, 12),
      colorStop: clampedProgress,
      marginBottom: interpolate(clampedProgress, 0, -2),
    },
    middleBar: {
      width: interpolate(clampedProgress, 18, 16),
      colorStop: clampedProgress,
    },
    bottomBar: {
      translateX: interpolate(clampedProgress, 0, -11.5),
      rotateDeg: interpolate(clampedProgress, 0, 45),
      width: interpolate(clampedProgress, 18, 12),
      colorStop: clampedProgress,
      marginTop: interpolate(clampedProgress, 4, 2),
    },
    container: {
      translateX: interpolate(clampedProgress, 0, -60),
    },
  }
}
