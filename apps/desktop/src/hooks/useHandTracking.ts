import { useState, useCallback, useRef, useEffect } from 'react';
import { usePaneStore } from '@/stores/pane';
import { HandPose } from '@/components/hands';
import type { PinchCoordinates, HandLandmarks } from '@/components/hands';

interface HandDataContext {
  activeHandPose: HandPose;
  pinchMidpoint: PinchCoordinates | null;
  primaryHandLandmarks: HandLandmarks | null;
  trackedHandsCount: number;
}

export const useHandTracking = () => {
  const [isHandTrackingActive, setIsHandTrackingActive] = useState(false);
  const [handData, setHandData] = useState<HandDataContext | null>(null);
  const [draggingPaneId, setDraggingPaneId] = useState<string | null>(null);
  
  const initialPinchPositionRef = useRef<{ x: number; y: number } | null>(null);
  const paneStartPosRef = useRef<{ x: number; y: number } | null>(null);
  const prevHandDataRef = useRef<HandDataContext | null>(null);
  
  const { panes, bringPaneToFront, updatePanePosition, activePaneId } = usePaneStore();

  const toggleHandTracking = useCallback(() => {
    const newState = !isHandTrackingActive;
    setIsHandTrackingActive(newState);
    if (!newState && draggingPaneId) {
      setDraggingPaneId(null);
      initialPinchPositionRef.current = null;
      paneStartPosRef.current = null;
    }
  }, [isHandTrackingActive, draggingPaneId]);

  const handleHandDataUpdate = useCallback((data: HandDataContext) => {
    if (
      !prevHandDataRef.current ||
      data.activeHandPose !== prevHandDataRef.current.activeHandPose ||
      data.trackedHandsCount !== prevHandDataRef.current.trackedHandsCount ||
      JSON.stringify(data.pinchMidpoint) !==
      JSON.stringify(prevHandDataRef.current.pinchMidpoint)
    ) {
      prevHandDataRef.current = data;
      setHandData(data);
    }
  }, []);

  useEffect(() => {
    const TITLE_BAR_HEIGHT = 32;
    
    if (
      !isHandTrackingActive ||
      !handData ||
      !handData.pinchMidpoint ||
      handData.trackedHandsCount === 0
    ) {
      if (draggingPaneId) {
        setDraggingPaneId(null);
        initialPinchPositionRef.current = null;
        paneStartPosRef.current = null;
      }
      return;
    }

    const { activeHandPose, pinchMidpoint } = handData;

    if (activeHandPose === HandPose.PINCH_CLOSED) {
      if (!draggingPaneId) {
        for (let i = panes.length - 1; i >= 0; i--) {
          const pane = panes[i];
          if (!pane) continue; // Skip if pane is undefined
          
          if (
            pinchMidpoint.x >= pane.x &&
            pinchMidpoint.x <= pane.x + pane.width &&
            pinchMidpoint.y >= pane.y &&
            pinchMidpoint.y <= pane.y + TITLE_BAR_HEIGHT
          ) {
            setDraggingPaneId(pane.id);
            paneStartPosRef.current = { x: pane.x, y: pane.y };
            initialPinchPositionRef.current = {
              x: pinchMidpoint.x,
              y: pinchMidpoint.y,
            };
            if (pane.id !== activePaneId) {
              bringPaneToFront(pane.id);
            }
            break;
          }
        }
      } else if (initialPinchPositionRef.current && paneStartPosRef.current) {
        const deltaX = pinchMidpoint.x - initialPinchPositionRef.current.x;
        const deltaY = pinchMidpoint.y - initialPinchPositionRef.current.y;

        if (Math.abs(deltaX) >= 1 || Math.abs(deltaY) >= 1) {
          const newX = paneStartPosRef.current.x + deltaX;
          const newY = paneStartPosRef.current.y + deltaY;

          initialPinchPositionRef.current = {
            x: pinchMidpoint.x,
            y: pinchMidpoint.y,
          };
          paneStartPosRef.current = { x: newX, y: newY };

          updatePanePosition(draggingPaneId, newX, newY);
        }
      }
    } else {
      if (draggingPaneId) {
        setDraggingPaneId(null);
        initialPinchPositionRef.current = null;
        paneStartPosRef.current = null;
      }
    }
  }, [
    isHandTrackingActive,
    handData,
    draggingPaneId,
    panes,
    activePaneId,
    bringPaneToFront,
    updatePanePosition,
  ]);

  return {
    isHandTrackingActive,
    toggleHandTracking,
    handleHandDataUpdate,
    handData,
  };
};