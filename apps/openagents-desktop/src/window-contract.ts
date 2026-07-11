/**
 * Window-control IPC channel names shared by main and preload.
 *
 * Owner contract (EP250 #8712): "add a hotkey for maximizing
 * (command+something) to fullscreen like command f" — the renderer never
 * receives a window handle; it invokes this channel and main toggles the
 * sender's BrowserWindow fullscreen state.
 */
export const DesktopWindowFullscreenChannel = "openagents-desktop/window-fullscreen-toggle" as const
