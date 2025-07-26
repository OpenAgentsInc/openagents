import { useEffect } from 'react';
import { usePaneStore } from '@/stores/pane';

interface KeyboardShortcutsProps {
  newProjectPath: string;
  createSession: () => void;
  toggleHandTracking: () => void;
}

export const useKeyboardShortcuts = ({
  newProjectPath,
  createSession,
  toggleHandTracking,
}: KeyboardShortcutsProps) => {
  const { 
    activePaneId, 
    removePane, 
    organizePanes, 
    toggleMetadataPane, 
    toggleSettingsPane, 
    toggleStatsPane 
  } = usePaneStore();

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') {
        return;
      }

      if (event.key === 'Escape' && activePaneId) {
        event.preventDefault();
        removePane(activePaneId);
        return;
      }

      const modifier = navigator.platform.toUpperCase().indexOf('MAC') >= 0
        ? event.metaKey
        : event.ctrlKey;

      if (!modifier) return;

      const digit = parseInt(event.key);
      if (isNaN(digit) || digit < 1 || digit > 9) return;

      event.preventDefault();

      switch (digit) {
        case 1:
          if (newProjectPath) {
            createSession();
          }
          break;
        case 2:
          organizePanes();
          break;
        case 3:
          toggleMetadataPane();
          break;
        case 4:
          toggleStatsPane();
          break;
        case 7:
          toggleSettingsPane();
          break;
        case 9:
          toggleHandTracking();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    toggleMetadataPane, 
    toggleSettingsPane, 
    toggleStatsPane, 
    organizePanes, 
    newProjectPath, 
    createSession, 
    toggleHandTracking, 
    activePaneId, 
    removePane
  ]);
};