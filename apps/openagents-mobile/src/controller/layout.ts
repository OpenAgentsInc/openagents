export const CONTROLLER_SPLIT_MIN_WIDTH = 720;
export const CONTROLLER_SPLIT_MIN_HEIGHT = 600;
export const CONTROLLER_CHAT_MAX_WIDTH = 960;

export type ControllerLayout = Readonly<{
  mode: "stack" | "split";
  sidebarWidth: number;
  chatWidth: number;
  useSheets: boolean;
}>;

export const controllerLayout = (width: number, height: number): ControllerLayout => {
  const safeWidth = Math.max(0, width);
  const safeHeight = Math.max(0, height);
  const split =
    safeWidth >= CONTROLLER_SPLIT_MIN_WIDTH && safeHeight >= CONTROLLER_SPLIT_MIN_HEIGHT;
  const sidebarWidth = split
    ? Math.min(460, Math.max(280, Math.round(safeWidth * 0.32)))
    : safeWidth;
  return {
    mode: split ? "split" : "stack",
    sidebarWidth,
    chatWidth: Math.min(
      CONTROLLER_CHAT_MAX_WIDTH,
      Math.max(0, safeWidth - (split ? sidebarWidth : 0)),
    ),
    useSheets: safeWidth < CONTROLLER_SPLIT_MIN_WIDTH,
  };
};
