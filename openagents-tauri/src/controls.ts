import { type KeyboardControlsEntry } from "@react-three/drei";

export enum AppControls {
  HOTBAR_1 = "HOTBAR_1", // Metadata Panel
  HOTBAR_2 = "HOTBAR_2", // New Chat
  HOTBAR_3 = "HOTBAR_3", // Future use
  HOTBAR_4 = "HOTBAR_4", // Future use
  HOTBAR_5 = "HOTBAR_5", // Future use
  HOTBAR_6 = "HOTBAR_6", // Future use
  HOTBAR_7 = "HOTBAR_7", // Future use
  HOTBAR_8 = "HOTBAR_8", // Settings
  HOTBAR_9 = "HOTBAR_9", // Help
}

export const appControlsMap: KeyboardControlsEntry<AppControls>[] = [
  { name: AppControls.HOTBAR_1, keys: ["Digit1", "Numpad1"] },
  { name: AppControls.HOTBAR_2, keys: ["Digit2", "Numpad2"] },
  { name: AppControls.HOTBAR_3, keys: ["Digit3", "Numpad3"] },
  { name: AppControls.HOTBAR_4, keys: ["Digit4", "Numpad4"] },
  { name: AppControls.HOTBAR_5, keys: ["Digit5", "Numpad5"] },
  { name: AppControls.HOTBAR_6, keys: ["Digit6", "Numpad6"] },
  { name: AppControls.HOTBAR_7, keys: ["Digit7", "Numpad7"] },
  { name: AppControls.HOTBAR_8, keys: ["Digit8", "Numpad8"] },
  { name: AppControls.HOTBAR_9, keys: ["Digit9", "Numpad9"] },
];