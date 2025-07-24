import { HandPose, type HandLandmarks, type Landmark } from "./handPoseTypes";

const LandmarkIndex = {
  WRIST: 0,
  THUMB_CMC: 1,
  THUMB_MCP: 2,
  THUMB_IP: 3,
  THUMB_TIP: 4,
  INDEX_FINGER_MCP: 5,
  INDEX_FINGER_PIP: 6,
  INDEX_FINGER_DIP: 7,
  INDEX_FINGER_TIP: 8,
  MIDDLE_FINGER_MCP: 9,
  MIDDLE_FINGER_PIP: 10,
  MIDDLE_FINGER_DIP: 11,
  MIDDLE_FINGER_TIP: 12,
  RING_FINGER_MCP: 13,
  RING_FINGER_PIP: 14,
  RING_FINGER_DIP: 15,
  RING_FINGER_TIP: 16,
  PINKY_MCP: 17,
  PINKY_PIP: 18,
  PINKY_DIP: 19,
  PINKY_TIP: 20,
};

function distance(p1: Landmark, p2: Landmark): number {
  return Math.sqrt(
    Math.pow(p1.x - p2.x, 2) +
      Math.pow(p1.y - p2.y, 2) +
      Math.pow(p1.z - p2.z, 2),
  );
}

function isFingerExtended(
  tip: Landmark,
  pip: Landmark,
  mcp: Landmark,
): boolean {
  const verticalCheck = tip.y < pip.y && pip.y < mcp.y;
  const straightCheck = distance(mcp, tip) > distance(mcp, pip) * 0.9;
  return verticalCheck && straightCheck;
}

function isFingerCurled(
  tip: Landmark,
  pip: Landmark,
  mcp: Landmark,
  wrist: Landmark,
): boolean {
  const tipLowerThanPip = tip.y > pip.y;
  const referenceDistance = distance(wrist, mcp);
  const tipToMcpDistance = distance(tip, mcp);
  const curledThreshold = referenceDistance * 0.7;

  return (
    tipLowerThanPip &&
    (tipToMcpDistance < curledThreshold ||
      tip.y - pip.y > referenceDistance * 0.1)
  );
}


function isPinchClosed(landmarks: HandLandmarks): boolean {
  const thumbTip = landmarks[LandmarkIndex.THUMB_TIP];
  const indexTip = landmarks[LandmarkIndex.INDEX_FINGER_TIP];
  const pinchDist = distance(thumbTip, indexTip);
  const pinchThreshold = 0.1;
  const closeFingers = pinchDist < pinchThreshold;
  
  // Simplified check for now, just look at distance
  return closeFingers;
}

function isFist(landmarks: HandLandmarks): boolean {
  const wrist = landmarks[LandmarkIndex.WRIST];
  const fingersCurled =
    isFingerCurled(
      landmarks[LandmarkIndex.INDEX_FINGER_TIP],
      landmarks[LandmarkIndex.INDEX_FINGER_PIP],
      landmarks[LandmarkIndex.INDEX_FINGER_MCP],
      wrist,
    ) &&
    isFingerCurled(
      landmarks[LandmarkIndex.MIDDLE_FINGER_TIP],
      landmarks[LandmarkIndex.MIDDLE_FINGER_PIP],
      landmarks[LandmarkIndex.MIDDLE_FINGER_MCP],
      wrist,
    ) &&
    isFingerCurled(
      landmarks[LandmarkIndex.RING_FINGER_TIP],
      landmarks[LandmarkIndex.RING_FINGER_PIP],
      landmarks[LandmarkIndex.RING_FINGER_MCP],
      wrist,
    ) &&
    isFingerCurled(
      landmarks[LandmarkIndex.PINKY_TIP],
      landmarks[LandmarkIndex.PINKY_PIP],
      landmarks[LandmarkIndex.PINKY_MCP],
      wrist,
    );

  if (!fingersCurled) return false;

  const thumbTip = landmarks[LandmarkIndex.THUMB_TIP];
  const thumbMcp = landmarks[LandmarkIndex.THUMB_MCP];
  const thumbPip = landmarks[LandmarkIndex.INDEX_FINGER_PIP];

  const thumbCurledOrAcross =
    thumbTip.y > thumbMcp.y ||
    distance(thumbTip, thumbPip) <
      distance(
        landmarks[LandmarkIndex.WRIST],
        landmarks[LandmarkIndex.THUMB_MCP],
      ) *
        0.8;

  return thumbCurledOrAcross;
}

function areAllFingersExtended(landmarks: HandLandmarks): boolean {
  return (
    isFingerExtended(
      landmarks[LandmarkIndex.INDEX_FINGER_TIP],
      landmarks[LandmarkIndex.INDEX_FINGER_PIP],
      landmarks[LandmarkIndex.INDEX_FINGER_MCP],
    ) &&
    isFingerExtended(
      landmarks[LandmarkIndex.MIDDLE_FINGER_TIP],
      landmarks[LandmarkIndex.MIDDLE_FINGER_PIP],
      landmarks[LandmarkIndex.MIDDLE_FINGER_MCP],
    ) &&
    isFingerExtended(
      landmarks[LandmarkIndex.RING_FINGER_TIP],
      landmarks[LandmarkIndex.RING_FINGER_PIP],
      landmarks[LandmarkIndex.RING_FINGER_MCP],
    ) &&
    isFingerExtended(
      landmarks[LandmarkIndex.PINKY_TIP],
      landmarks[LandmarkIndex.PINKY_PIP],
      landmarks[LandmarkIndex.PINKY_MCP],
    ) &&
    isFingerExtended(
      landmarks[LandmarkIndex.THUMB_TIP],
      landmarks[LandmarkIndex.THUMB_IP],
      landmarks[LandmarkIndex.THUMB_MCP],
    )
  );
}

function isFlatHand(landmarks: HandLandmarks): boolean {
  if (!areAllFingersExtended(landmarks)) {
    return false;
  }

  const indexTip = landmarks[LandmarkIndex.INDEX_FINGER_TIP];
  const pinkyTip = landmarks[LandmarkIndex.PINKY_TIP];
  const indexMcp = landmarks[LandmarkIndex.INDEX_FINGER_MCP];
  const pinkyMcp = landmarks[LandmarkIndex.PINKY_MCP];

  const tipSpread = distance(indexTip, pinkyTip);
  const mcpSpread = distance(indexMcp, pinkyMcp);

  return tipSpread < mcpSpread * 1.7;
}

function isOpenHand(landmarks: HandLandmarks): boolean {
  if (!areAllFingersExtended(landmarks)) {
    return false;
  }
  
  const indexTip = landmarks[LandmarkIndex.INDEX_FINGER_TIP];
  const pinkyTip = landmarks[LandmarkIndex.PINKY_TIP];
  const indexMcp = landmarks[LandmarkIndex.INDEX_FINGER_MCP];
  const pinkyMcp = landmarks[LandmarkIndex.PINKY_MCP];

  const tipSpread = distance(indexTip, pinkyTip);
  const mcpSpread = distance(indexMcp, pinkyMcp);

  return tipSpread > mcpSpread * 1.6;
}

function isTwoFingerV(landmarks: HandLandmarks): boolean {
  const wrist = landmarks[LandmarkIndex.WRIST];
  const indexExtended = isFingerExtended(
    landmarks[LandmarkIndex.INDEX_FINGER_TIP],
    landmarks[LandmarkIndex.INDEX_FINGER_PIP],
    landmarks[LandmarkIndex.INDEX_FINGER_MCP],
  );
  const middleExtended = isFingerExtended(
    landmarks[LandmarkIndex.MIDDLE_FINGER_TIP],
    landmarks[LandmarkIndex.MIDDLE_FINGER_PIP],
    landmarks[LandmarkIndex.MIDDLE_FINGER_MCP],
  );

  const ringCurled = isFingerCurled(
    landmarks[LandmarkIndex.RING_FINGER_TIP],
    landmarks[LandmarkIndex.RING_FINGER_PIP],
    landmarks[LandmarkIndex.RING_FINGER_MCP],
    wrist,
  );
  const pinkyCurled = isFingerCurled(
    landmarks[LandmarkIndex.PINKY_TIP],
    landmarks[LandmarkIndex.PINKY_PIP],
    landmarks[LandmarkIndex.PINKY_MCP],
    wrist,
  );

  if (indexExtended && middleExtended && ringCurled && pinkyCurled) {
    const indexTip = landmarks[LandmarkIndex.INDEX_FINGER_TIP];
    const middleTip = landmarks[LandmarkIndex.MIDDLE_FINGER_TIP];
    const wristToIndexMcp = distance(
      landmarks[LandmarkIndex.WRIST],
      landmarks[LandmarkIndex.INDEX_FINGER_MCP],
    );

    const vSpreadThreshold = wristToIndexMcp * 0.3;
    return distance(indexTip, middleTip) > vSpreadThreshold;
  }
  return false;
}

export function recognizeHandPose(landmarks: HandLandmarks | null): HandPose {
  if (!landmarks || landmarks.length < 21) {
    return HandPose.NONE;
  }

  if (isPinchClosed(landmarks)) {
    return HandPose.PINCH_CLOSED;
  }

  if (isFist(landmarks)) {
    return HandPose.FIST;
  }
  if (isTwoFingerV(landmarks)) {
    return HandPose.TWO_FINGER_V;
  }
  if (isOpenHand(landmarks)) {
    return HandPose.OPEN_HAND;
  }
  if (isFlatHand(landmarks)) {
    return HandPose.FLAT_HAND;
  }

  return HandPose.NONE;
}