import { useEffect } from 'react';
import { usePaneStore } from '@/stores/pane';
import { useHotbarStore } from '@/stores/hotbar';

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
  
  const { setPressedSlot } = useHotbarStore();

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
      
      // Set the slot as pressed
      setPressedSlot(digit, true);
      
      // Add a small delay before executing the action for visual feedback
      setTimeout(() => {
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
      }, 50);
    };
    
    const handleKeyUp = (event: KeyboardEvent) => {
      const modifier = navigator.platform.toUpperCase().indexOf('MAC') >= 0
        ? event.metaKey
        : event.ctrlKey;
        
      const digit = parseInt(event.key);
      if (!isNaN(digit) && digit >= 1 && digit <= 9) {
        // Release the pressed state after a short delay for better visual feedback
        setTimeout(() => {
          setPressedSlot(digit, false);
        }, 100);
      }
      
      // Also handle when modifier key is released
      if (event.key === 'Meta' || event.key === 'Control') {
        // Clear all pressed slots when modifier is released
        for (let i = 1; i <= 9; i++) {
          setPressedSlot(i, false);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, [
    toggleMetadataPane, 
    toggleSettingsPane, 
    toggleStatsPane, 
    organizePanes, 
    newProjectPath, 
    createSession, 
    toggleHandTracking, 
    activePaneId, 
    removePane,
    setPressedSlot
  ]);
};