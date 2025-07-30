import { HandLandmarks, PinchCoordinates } from "./handPoseTypes";

interface SmoothedLandmark {
  x: number;
  y: number;
  z: number;
  visibility?: number;
  vx: number; // velocity x
  vy: number; // velocity y
  vz: number; // velocity z
}

export class HandSmoother {
  private smoothedLandmarks: Map<number, SmoothedLandmark[]> = new Map();
  private lastUpdateTime: Map<number, number> = new Map();
  
  // Smoothing parameters
  private readonly smoothingFactor = 0.3; // Lower = smoother, higher = more responsive
  private readonly velocitySmoothingFactor = 0.2;
  private readonly minDistanceThreshold = 0.001; // Minimum movement to update
  private readonly maxVelocity = 0.5; // Maximum velocity per second
  
  constructor() {}

  /**
   * Smooth hand landmarks using exponential moving average with velocity estimation
   */
  smoothHandLandmarks(
    handIndex: number,
    rawLandmarks: HandLandmarks,
    currentTime: number = performance.now()
  ): HandLandmarks {
    const lastTime = this.lastUpdateTime.get(handIndex) || currentTime;
    const deltaTime = Math.min((currentTime - lastTime) / 1000, 0.1); // Cap at 100ms
    this.lastUpdateTime.set(handIndex, currentTime);

    let smoothed = this.smoothedLandmarks.get(handIndex);
    
    if (!smoothed || smoothed.length !== rawLandmarks.length) {
      // Initialize smoothed landmarks
      smoothed = rawLandmarks.map(landmark => ({
        ...landmark,
        vx: 0,
        vy: 0,
        vz: 0,
      }));
      this.smoothedLandmarks.set(handIndex, smoothed);
      return rawLandmarks;
    }

    // Apply smoothing with velocity estimation
    const result = smoothed.map((smooth, i) => {
      const raw = rawLandmarks[i];
      if (!raw) {
        return smooth; // Return existing smooth data if raw data is missing
      }
      
      // Calculate raw velocity
      const rawVx = (raw.x - smooth.x) / Math.max(deltaTime, 0.016); // 60fps minimum
      const rawVy = (raw.y - smooth.y) / Math.max(deltaTime, 0.016);
      const rawVz = (raw.z - smooth.z) / Math.max(deltaTime, 0.016);
      
      // Smooth velocities
      smooth.vx = smooth.vx * (1 - this.velocitySmoothingFactor) + 
                  rawVx * this.velocitySmoothingFactor;
      smooth.vy = smooth.vy * (1 - this.velocitySmoothingFactor) + 
                  rawVy * this.velocitySmoothingFactor;
      smooth.vz = smooth.vz * (1 - this.velocitySmoothingFactor) + 
                  rawVz * this.velocitySmoothingFactor;
      
      // Clamp velocities
      const velocityMagnitude = Math.sqrt(smooth.vx ** 2 + smooth.vy ** 2 + smooth.vz ** 2);
      if (velocityMagnitude > this.maxVelocity) {
        const scale = this.maxVelocity / velocityMagnitude;
        smooth.vx *= scale;
        smooth.vy *= scale;
        smooth.vz *= scale;
      }
      
      // Predict next position based on velocity
      const predictedX = smooth.x + smooth.vx * deltaTime;
      const predictedY = smooth.y + smooth.vy * deltaTime;
      const predictedZ = smooth.z + smooth.vz * deltaTime;
      
      // Blend between predicted and raw position
      const distance = Math.sqrt(
        (raw.x - smooth.x) ** 2 + 
        (raw.y - smooth.y) ** 2 + 
        (raw.z - smooth.z) ** 2
      );
      
      // Adaptive smoothing based on movement distance
      const adaptiveFactor = distance > 0.05 ? 
        this.smoothingFactor * 2 : // Less smoothing for large movements
        this.smoothingFactor;
      
      if (distance > this.minDistanceThreshold) {
        smooth.x = predictedX * (1 - adaptiveFactor) + raw.x * adaptiveFactor;
        smooth.y = predictedY * (1 - adaptiveFactor) + raw.y * adaptiveFactor;
        smooth.z = predictedZ * (1 - adaptiveFactor) + raw.z * adaptiveFactor;
      }
      
      if (raw.visibility !== undefined) {
        smooth.visibility = smooth.visibility !== undefined ?
          smooth.visibility * 0.8 + raw.visibility * 0.2 :
          raw.visibility;
      }
      
      return {
        x: smooth.x,
        y: smooth.y,
        z: smooth.z,
        visibility: smooth.visibility,
      };
    });

    return result;
  }

  /**
   * Smooth pinch coordinates with extra stabilization for dragging
   */
  smoothPinchCoordinates(
    pinch: PinchCoordinates | null,
    handIndex: number = 0
  ): PinchCoordinates | null {
    if (!pinch) return null;

    const pinchKey = 1000 + handIndex; // Use high number to avoid collision with hand indices
    const smoothed = this.smoothedLandmarks.get(pinchKey)?.[0];
    
    if (!smoothed) {
      this.smoothedLandmarks.set(pinchKey, [{
        x: pinch.x,
        y: pinch.y,
        z: pinch.z,
        vx: 0,
        vy: 0,
        vz: 0,
      }]);
      return pinch;
    }

    // Extra stabilization for pinch (less responsive but smoother)
    const pinchSmoothingFactor = 0.15;
    
    smoothed.x = smoothed.x * (1 - pinchSmoothingFactor) + pinch.x * pinchSmoothingFactor;
    smoothed.y = smoothed.y * (1 - pinchSmoothingFactor) + pinch.y * pinchSmoothingFactor;
    smoothed.z = smoothed.z * (1 - pinchSmoothingFactor) + pinch.z * pinchSmoothingFactor;

    return {
      x: smoothed.x,
      y: smoothed.y,
      z: smoothed.z,
      normalizedMidX: pinch.normalizedMidX,
      normalizedMidY: pinch.normalizedMidY,
    };
  }

  /**
   * Clear smoothing data for a hand
   */
  clearHand(handIndex: number) {
    this.smoothedLandmarks.delete(handIndex);
    this.lastUpdateTime.delete(handIndex);
  }

  /**
   * Clear all smoothing data
   */
  clear() {
    this.smoothedLandmarks.clear();
    this.lastUpdateTime.clear();
  }
}