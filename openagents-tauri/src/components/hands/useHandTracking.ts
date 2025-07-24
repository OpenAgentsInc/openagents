import { useCallback, useEffect, useRef, useState } from "react";
import { Camera } from "@mediapipe/camera_utils";
import {
  Hands,
  Results as HandResults,
  HAND_CONNECTIONS,
} from "@mediapipe/hands";
import {
  HandPose,
  type HandLandmarks,
  type PinchCoordinates,
} from "./handPoseTypes";
import { recognizeHandPose } from "./handPoseRecognition";
import { HandSmoother } from "./HandSmoother";
import { HandRenderer } from "./HandRenderer";

interface TrackedHandInfo {
  landmarks: HandLandmarks;
  pose: HandPose;
  pinchMidpoint: PinchCoordinates | null;
  handedness: string;
}

declare global {
  interface Window {
    moduleInitialized: boolean;
  }
}

export interface HandPosition {
  x: number;
  y: number;
}

interface UseHandTrackingOptions {
  enabled: boolean;
}

export function useHandTracking({ enabled }: UseHandTrackingOptions) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const landmarkCanvasRef = useRef<HTMLCanvasElement>(null);
  const cameraRef = useRef<Camera | null>(null);
  const handsRef = useRef<Hands | null>(null);
  const smootherRef = useRef<HandSmoother | null>(null);
  const rendererRef = useRef<HandRenderer | null>(null);
  const [handTrackingStatus, setHandTrackingStatus] = useState("Inactive");
  const [handPosition, setHandPosition] = useState<HandPosition | null>(null);
  const [trackedHands, setTrackedHands] = useState<TrackedHandInfo[]>([]);

  const onHandTrackingResults = useCallback(
    (results: HandResults) => {
      if (!landmarkCanvasRef.current || !enabled) {
        if (rendererRef.current) {
          rendererRef.current.clear();
        }
        setHandPosition(null);
        setTrackedHands([]);
        return;
      }

      // Initialize smoother and renderer if needed
      if (!smootherRef.current) {
        smootherRef.current = new HandSmoother();
      }
      if (!rendererRef.current) {
        rendererRef.current = new HandRenderer(landmarkCanvasRef.current);
      }

      // Clear canvas
      rendererRef.current.clear();

      let handsDetected = 0;
      const currentFrameTrackedHands: TrackedHandInfo[] = [];

      if (results.multiHandLandmarks && results.multiHandedness) {
        handsDetected = results.multiHandLandmarks.length;
        for (
          let index = 0;
          index < results.multiHandLandmarks.length;
          index++
        ) {
          const classification = results.multiHandedness[index];
          const handedness = classification.label;
          const rawLandmarks = results.multiHandLandmarks[index] as HandLandmarks;
          
          // Apply smoothing to landmarks
          const landmarks = smootherRef.current!.smoothHandLandmarks(index, rawLandmarks);
          const pose = recognizeHandPose(landmarks);

          let currentPinchMidpoint: PinchCoordinates | null = null;
          if (pose === HandPose.PINCH_CLOSED) {
            const thumbTip = landmarks[4];
            const indexTip = landmarks[8];

            if (thumbTip && indexTip && landmarkCanvasRef.current) {
              const normalizedMidX = (thumbTip.x + indexTip.x) / 2;
              const normalizedMidY = (thumbTip.y + indexTip.y) / 2;

              const mirroredNormalizedMidX = 1 - normalizedMidX;
              const screenPinchX = mirroredNormalizedMidX * window.innerWidth;
              const screenPinchY = normalizedMidY * window.innerHeight;

              currentPinchMidpoint = {
                x: screenPinchX,
                y: screenPinchY,
                z: (thumbTip.z + indexTip.z) / 2,
              };

              currentPinchMidpoint.normalizedMidX = normalizedMidX;
              currentPinchMidpoint.normalizedMidY = normalizedMidY;
              
              // Apply smoothing to pinch coordinates
              currentPinchMidpoint = smootherRef.current!.smoothPinchCoordinates(
                currentPinchMidpoint,
                index
              );
            }
          }

          currentFrameTrackedHands.push({
            landmarks,
            pose,
            pinchMidpoint: currentPinchMidpoint,
            handedness,
          });

          if (index === 0) {
            if (landmarks.length > 8) {
              const indexFingerTip = landmarks[8];
              setHandPosition({
                x: indexFingerTip.x,
                y: indexFingerTip.y,
              });
            }
          }

          // Render hand using custom renderer
          rendererRef.current!.renderHand(landmarks, HAND_CONNECTIONS);
        }
      }

      // Render pinch indicators after all hands
      currentFrameTrackedHands.forEach((hand) => {
        if (hand.pinchMidpoint) {
          const isPinching = hand.pose === HandPose.PINCH_CLOSED;
          rendererRef.current!.renderPinch(
            hand.pinchMidpoint,
            isPinching,
            hand.handedness
          );
        }
      });

      setTrackedHands(currentFrameTrackedHands);

      if (currentFrameTrackedHands.length === 0) {
        setHandPosition(null);
      }

      if (enabled) {
        setHandTrackingStatus(
          handsDetected > 0
            ? `${handsDetected} hand(s) detected`
            : "No hands detected",
        );
      }
    },
    [enabled],
  );

  useEffect(() => {
    const cleanupMediaPipe = () => {
      if (cameraRef.current) {
        try {
          cameraRef.current.stop();
        } catch (err) {
          console.error("Error stopping camera:", err);
        }
        cameraRef.current = null;
      }

      if (handsRef.current) {
        try {
          handsRef.current.close();
        } catch (err) {
          console.error("Error closing MediaPipe Hands:", err);
        }
        handsRef.current = null;
      }

      if (videoRef.current && videoRef.current.srcObject) {
        try {
          const stream = videoRef.current.srcObject as MediaStream;
          stream.getTracks().forEach((track) => track.stop());
          videoRef.current.srcObject = null;
          videoRef.current.load();
        } catch (err) {
          console.error("Error cleaning up video stream:", err);
        }
      }

      if (landmarkCanvasRef.current) {
        const canvasCtx = landmarkCanvasRef.current.getContext("2d");
        if (canvasCtx) {
          canvasCtx.clearRect(
            0,
            0,
            landmarkCanvasRef.current.width,
            landmarkCanvasRef.current.height,
          );
        }
      }

      setHandTrackingStatus("Inactive");
      setHandPosition(null);
      setTrackedHands([]);
      
      // Clear smoother data
      if (smootherRef.current) {
        smootherRef.current.clear();
        smootherRef.current = null;
      }
      
      // Clear renderer
      if (rendererRef.current) {
        rendererRef.current.clear();
        rendererRef.current = null;
      }
    };

    if (!enabled) {
      cleanupMediaPipe();
      return;
    }

    if (!videoRef.current || !landmarkCanvasRef.current) {
      return;
    }

    window.moduleInitialized = false;

    setHandTrackingStatus("Initializing MediaPipe...");

    if (handsRef.current) {
      try {
        handsRef.current.close();
      } catch (e) {
        /* ignore */
      }
      handsRef.current = null;
    }

    if (cameraRef.current) {
      try {
        cameraRef.current.stop();
      } catch (e) {
        /* ignore */
      }
      cameraRef.current = null;
    }

    try {
      handsRef.current = new Hands({
        locateFile: (file) => {
          return `/mediapipe/hands/${file}`;
        },
      });

      handsRef.current.setOptions({
        selfieMode: false,
        maxNumHands: 2,
        modelComplexity: 0,
        minDetectionConfidence: 0.7,
        minTrackingConfidence: 0.5,
      });

      handsRef.current.onResults(onHandTrackingResults);
      setHandTrackingStatus("MediaPipe initialized");

      cameraRef.current = new Camera(videoRef.current, {
        onFrame: async () => {
          if (
            videoRef.current &&
            handsRef.current &&
            videoRef.current.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA
          ) {
            try {
              await handsRef.current.send({ image: videoRef.current });
            } catch (err) {
              // Expected during shutdown
            }
          }
        },
        width: 640,
        height: 480,
      });

      cameraRef.current.start();
      setHandTrackingStatus("Tracking active");
    } catch (error) {
      console.error("Init error:", error);
      setHandTrackingStatus(
        `Error initializing MediaPipe: ${error instanceof Error ? error.message : String(error)}`,
      );
      cleanupMediaPipe();
    }

    return () => {
      cleanupMediaPipe();
    };
  }, [enabled, onHandTrackingResults]);

  useEffect(() => {
    if (!enabled) return;

    const updateCanvasDimensions = () => {
      if (videoRef.current && landmarkCanvasRef.current) {
        const videoWidth =
          videoRef.current.videoWidth || videoRef.current.clientWidth;
        const videoHeight =
          videoRef.current.videoHeight || videoRef.current.clientHeight;

        if (videoWidth > 0 && videoHeight > 0) {
          landmarkCanvasRef.current.width = videoWidth;
          landmarkCanvasRef.current.height = videoHeight;
        } else {
          landmarkCanvasRef.current.width = videoRef.current.offsetWidth;
          landmarkCanvasRef.current.height = videoRef.current.offsetHeight;
        }
        
        // Update renderer with new canvas
        if (rendererRef.current && landmarkCanvasRef.current) {
          rendererRef.current = new HandRenderer(landmarkCanvasRef.current);
        }
      }
    };

    const videoEl = videoRef.current;
    if (videoEl) {
      videoEl.addEventListener("loadedmetadata", updateCanvasDimensions);
      videoEl.addEventListener("play", updateCanvasDimensions);
    }

    window.addEventListener("resize", updateCanvasDimensions);
    updateCanvasDimensions();

    return () => {
      if (videoEl) {
        videoEl.removeEventListener("loadedmetadata", updateCanvasDimensions);
        videoEl.removeEventListener("play", updateCanvasDimensions);
      }
      window.removeEventListener("resize", updateCanvasDimensions);
    };
  }, [enabled]);

  const activeHand = trackedHands.length > 0 ? trackedHands[0] : null;
  const activeHandPose = activeHand ? activeHand.pose : HandPose.NONE;
  const pinchMidpoint = activeHand ? activeHand.pinchMidpoint : null;

  return {
    videoRef,
    landmarkCanvasRef,
    handPosition,
    handTrackingStatus,
    activeHandPose,
    pinchMidpoint,
    trackedHands,
  };
}