import { useEffect } from 'react';

export type KeyboardShortcut = {
  key: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  handler: () => void;
};

export function useKeyboardShortcuts(shortcuts: KeyboardShortcut[]) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Check if the user is typing in an input field
      const target = event.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') {
        return;
      }

      for (const shortcut of shortcuts) {
        const isModifierPressed = navigator.platform.toUpperCase().indexOf('MAC') >= 0
          ? event.metaKey // Mac uses Cmd key
          : event.ctrlKey; // Windows/Linux use Ctrl key

        if (
          event.key === shortcut.key &&
          (shortcut.ctrlKey === undefined || event.ctrlKey === shortcut.ctrlKey) &&
          (shortcut.metaKey === undefined || event.metaKey === shortcut.metaKey) &&
          (shortcut.shiftKey === undefined || event.shiftKey === shortcut.shiftKey) &&
          (shortcut.altKey === undefined || event.altKey === shortcut.altKey) &&
          (shortcut.ctrlKey || shortcut.metaKey ? isModifierPressed : true)
        ) {
          event.preventDefault();
          shortcut.handler();
          break;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [shortcuts]);
}