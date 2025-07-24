import { useCallback, useEffect, useRef, useState } from "react";
import { Camera } from "@mediapipe/camera_utils";
import {
  Hands,
  Results as HandResults,
  LandmarkConnectionArray,
  HAND_CONNECTIONS,
} from "@mediapipe/hands";
import { drawConnectors, drawLandmarks } from "@mediapipe/drawing_utils";
import {
  HandPose,
  type HandLandmarks,
  type PinchCoordinates,
} from "./handPoseTypes";
import { recognizeHandPose } from "./handPoseRecognition";

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
  const [handTrackingStatus, setHandTrackingStatus] = useState("Inactive");
  const [handPosition, setHandPosition] = useState<HandPosition | null>(null);
  const [trackedHands, setTrackedHands] = useState<TrackedHandInfo[]>([]);

  const onHandTrackingResults = useCallback(
    (results: HandResults) => {
      if (!landmarkCanvasRef.current || !enabled) {
        if (landmarkCanvasRef.current) {
          const canvasCtx = landmarkCanvasRef.current.getContext("2d")!;
          canvasCtx.clearRect(
            0,
            0,
            landmarkCanvasRef.current.width,
            landmarkCanvasRef.current.height,
          );
        }
        setHandPosition(null);
        setTrackedHands([]);
        return;
      }

      const canvasCtx = landmarkCanvasRef.current.getContext("2d")!;
      canvasCtx.save();
      canvasCtx.clearRect(
        0,
        0,
        landmarkCanvasRef.current.width,
        landmarkCanvasRef.current.height,
      );

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
          const landmarks = results.multiHandLandmarks[index] as HandLandmarks;
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

          drawConnectors(
            canvasCtx,
            landmarks,
            HAND_CONNECTIONS as LandmarkConnectionArray,
            {
              color: "#3f3f46",
              lineWidth: 1,
            },
          );

          drawLandmarks(canvasCtx, landmarks, {
            color: "#fff",
            lineWidth: 1,
            fillColor: "#000",
            radius: 4,
          });

          if (landmarks.length > 8) {
            const thumbTip = landmarks[4];
            const indexTip = landmarks[8];

            canvasCtx.beginPath();
            canvasCtx.arc(
              thumbTip.x * landmarkCanvasRef.current.width,
              thumbTip.y * landmarkCanvasRef.current.height,
              6,
              0,
              2 * Math.PI,
            );
            canvasCtx.fillStyle = "#ffffff";
            canvasCtx.fill();

            canvasCtx.beginPath();
            canvasCtx.arc(
              indexTip.x * landmarkCanvasRef.current.width,
              indexTip.y * landmarkCanvasRef.current.height,
              6,
              0,
              2 * Math.PI,
            );
            canvasCtx.fillStyle = "#ffffff";
            canvasCtx.fill();
          }
        }
      }

      currentFrameTrackedHands.forEach((hand) => {
        if (
          hand.pinchMidpoint &&
          hand.pinchMidpoint.normalizedMidX !== undefined &&
          hand.pinchMidpoint.normalizedMidY !== undefined
        ) {
          const normalizedMidX = hand.pinchMidpoint.normalizedMidX;
          const normalizedMidY = hand.pinchMidpoint.normalizedMidY;
          const screenPinchX = hand.pinchMidpoint.x;
          const screenPinchY = hand.pinchMidpoint.y;

          const canvas = landmarkCanvasRef.current!;
          const ctx = canvasCtx;
          const canvasWidth = canvas.width;
          const canvasHeight = canvas.height;

          const canvasDrawX_unmirrored = normalizedMidX * canvasWidth;
          const canvasDrawY_unmirrored = normalizedMidY * canvasHeight;

          ctx.beginPath();
          ctx.arc(
            canvasDrawX_unmirrored,
            canvasDrawY_unmirrored,
            10,
            0,
            2 * Math.PI,
          );
          ctx.strokeStyle = "rgba(255, 255, 255, 0.9)";
          ctx.lineWidth = 2;
          ctx.stroke();

          const coordText = `Pinch: ${Math.round(screenPinchX)}, ${Math.round(screenPinchY)} px`;

          ctx.save();
          ctx.scale(-1, 1);

          ctx.font = "bold 9px monospace";
          const textMetrics = ctx.measureText(coordText);
          const textWidth = textMetrics.width;
          const textHeight = 14;

          const visualCircleCenterX = (1 - normalizedMidX) * canvasWidth;
          const circleRadius = 10;
          const padding = 5;
          const visualTextAnchorLeftX =
            visualCircleCenterX + circleRadius + padding;
          const drawTextAnchorLeftX_in_flipped_ctx = -(
            canvasWidth - visualTextAnchorLeftX
          );
          const textY = canvasDrawY_unmirrored;

          ctx.textAlign = "left";

          ctx.fillStyle = "rgba(0, 0, 0, 0.75)";
          ctx.fillRect(
            drawTextAnchorLeftX_in_flipped_ctx - 2,
            textY - textHeight,
            textWidth + 4,
            textHeight + 4,
          );

          ctx.fillStyle = "rgba(255, 255, 255, 1)";
          ctx.fillText(
            coordText,
            drawTextAnchorLeftX_in_flipped_ctx,
            textY - 3,
          );

          ctx.restore();
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
      canvasCtx.restore();
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