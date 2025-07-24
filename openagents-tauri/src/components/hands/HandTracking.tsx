import { useEffect, useRef } from "react";
import { useHandTracking } from "./useHandTracking";
import {
  HandPose,
  type PinchCoordinates,
  type HandLandmarks,
} from "./handPoseTypes";

interface HandDataForCallback {
  activeHandPose: HandPose;
  pinchMidpoint: PinchCoordinates | null;
  primaryHandLandmarks: HandLandmarks | null;
  trackedHandsCount: number;
}

interface HandTrackingProps {
  showHandTracking: boolean;
  setShowHandTracking: (show: boolean) => void;
  onHandDataUpdate?: (data: HandDataForCallback) => void;
}

export default function HandTracking({
  showHandTracking,
  setShowHandTracking: _setShowHandTracking,
  onHandDataUpdate,
}: HandTrackingProps) {
  const {
    videoRef,
    landmarkCanvasRef,
    handPosition: _handPosition,
    handTrackingStatus: _handTrackingStatus,
    activeHandPose: _activeHandPose,
    pinchMidpoint: _pinchMidpoint,
    trackedHands,
  } = useHandTracking({ enabled: showHandTracking });

  const prevHandDataRef = useRef<{
    activeHandPose: HandPose;
    pinchCoords: string | null;
    trackedHandsCount: number;
  }>({
    activeHandPose: HandPose.NONE,
    pinchCoords: null,
    trackedHandsCount: 0,
  });

  useEffect(() => {
    if (!onHandDataUpdate) return;

    let data: HandDataForCallback;

    if (showHandTracking) {
      const primaryHand = trackedHands.length > 0 ? trackedHands[0] : null;
      const currentHandPose = primaryHand ? primaryHand.pose : HandPose.NONE;
      const currentPinchMidpoint = primaryHand
        ? primaryHand.pinchMidpoint
        : null;

      const currentPinchCoords = currentPinchMidpoint
        ? `${Math.round(currentPinchMidpoint.x)},${Math.round(currentPinchMidpoint.y)}`
        : null;

      const hasChanged =
        currentHandPose !== prevHandDataRef.current.activeHandPose ||
        currentPinchCoords !== prevHandDataRef.current.pinchCoords ||
        trackedHands.length !== prevHandDataRef.current.trackedHandsCount;

      if (hasChanged) {
        data = {
          activeHandPose: currentHandPose,
          pinchMidpoint: currentPinchMidpoint,
          primaryHandLandmarks: primaryHand ? primaryHand.landmarks : null,
          trackedHandsCount: trackedHands.length,
        };

        prevHandDataRef.current = {
          activeHandPose: currentHandPose,
          pinchCoords: currentPinchCoords,
          trackedHandsCount: trackedHands.length,
        };

        onHandDataUpdate(data);
      }
    } else if (
      prevHandDataRef.current.activeHandPose !== HandPose.NONE ||
      prevHandDataRef.current.trackedHandsCount !== 0
    ) {
      data = {
        activeHandPose: HandPose.NONE,
        pinchMidpoint: null,
        primaryHandLandmarks: null,
        trackedHandsCount: 0,
      };

      prevHandDataRef.current = {
        activeHandPose: HandPose.NONE,
        pinchCoords: null,
        trackedHandsCount: 0,
      };

      onHandDataUpdate(data);
    }
  }, [trackedHands, onHandDataUpdate, showHandTracking]);

  return (
    <>
      {showHandTracking && (
        <video
          ref={videoRef}
          autoPlay
          playsInline
          muted
          className="absolute h-full w-full scale-x-[-1] transform object-cover"
          style={{
            top: 0,
            left: 0,
            zIndex: 1,
            opacity: 0,
            pointerEvents: "none",
          }}
        />
      )}

      <canvas
        ref={landmarkCanvasRef}
        className="absolute h-full w-full scale-x-[-1] transform"
        style={{
          top: 0,
          left: 0,
          pointerEvents: "none",
          zIndex: 9999,
          visibility: showHandTracking ? "visible" : "hidden",
        }}
      />
    </>
  );
}