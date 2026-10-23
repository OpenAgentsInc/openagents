/** Navigation and tool choices only. Saving and publishing use their original controls. */
export const actionNames = {
  "world-edit": "Open editor",
  "edit-close": "Inspect world",
  "world-map": "Back to planets",
  "world-mesh": "Toggle mesh overlay",
  "world-fullscreen": "Full screen",
  "tab-objects": "Object tools",
  "tab-materials": "Material tools",
  "tab-request": "Request a change",
  "tab-compile": "World effect tools",
  "tab-history": "History and saved projects",
  "request-options": "Show options",
  "visit-toggle": "Show or hide chat",
  "verse-explore": "Explore planets",
  "planet-setup": "Set up my planet",
  "planet-enter": "Enter your courtyard",
} as const;
export type ActionId = keyof typeof actionNames;
export type AvailableAction = { id: ActionId; label: string };
export function availableActions(document: Document): AvailableAction[] {
  return Object.entries(actionNames).flatMap(([id, label]) => {
    const node = document.getElementById(id);
    return node &&
      !node.matches(":disabled") &&
      !node.closest("[hidden],[inert]") &&
      node.getClientRects().length
      ? [{ id: id as ActionId, label }]
      : [];
  });
}
export function invokeAction(document: Document, id: ActionId): void {
  const node = document.getElementById(id);
  if (
    !(id in actionNames) ||
    !node ||
    !availableActions(document).some((a) => a.id === id)
  )
    return;
  node.focus();
  node.click();
}
