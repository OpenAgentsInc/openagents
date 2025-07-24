import { HandLandmarks, PinchCoordinates } from "./handPoseTypes";
import { HAND_CONNECTIONS, LandmarkConnectionArray } from "@mediapipe/hands";

interface RenderOptions {
  jointColor?: string;
  jointRadius?: number;
  lineColor?: string;
  lineWidth?: number;
  pinchColor?: string;
  pinchRadius?: number;
  showPinchCoords?: boolean;
  minVisibility?: number;
}

const DEFAULT_OPTIONS: RenderOptions = {
  jointColor: "#ffffff",
  jointRadius: 4,
  lineColor: "#3f3f46",
  lineWidth: 2,
  pinchColor: "#10b981",
  pinchRadius: 12,
  showPinchCoords: true,
  minVisibility: 0.5,
};

export class HandRenderer {
  private ctx: CanvasRenderingContext2D;
  private canvas: HTMLCanvasElement;
  private options: RenderOptions;

  constructor(canvas: HTMLCanvasElement, options: Partial<RenderOptions> = {}) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("Could not get 2D context from canvas");
    this.ctx = ctx;
    this.options = { ...DEFAULT_OPTIONS, ...options };
  }

  /**
   * Clear the canvas
   */
  clear() {
    this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
  }

  /**
   * Render hand landmarks with smooth curves
   */
  renderHand(landmarks: HandLandmarks, connections: LandmarkConnectionArray = HAND_CONNECTIONS) {
    if (!landmarks || landmarks.length === 0) return;

    const { width, height } = this.canvas;
    
    // Draw connections first (behind joints)
    this.ctx.save();
    
    // Enable anti-aliasing
    this.ctx.imageSmoothingEnabled = true;
    this.ctx.imageSmoothingQuality = "high";
    
    // Draw connections with bezier curves for smoother lines
    connections.forEach(([start, end]) => {
      const startLandmark = landmarks[start];
      const endLandmark = landmarks[end];
      
      if (!startLandmark || !endLandmark) return;
      if (startLandmark.visibility && startLandmark.visibility < this.options.minVisibility!) return;
      if (endLandmark.visibility && endLandmark.visibility < this.options.minVisibility!) return;

      const startX = startLandmark.x * width;
      const startY = startLandmark.y * height;
      const endX = endLandmark.x * width;
      const endY = endLandmark.y * height;

      // Calculate control points for bezier curve
      const midX = (startX + endX) / 2;
      const midY = (startY + endY) / 2;
      
      this.ctx.beginPath();
      this.ctx.strokeStyle = this.options.lineColor!;
      this.ctx.lineWidth = this.options.lineWidth!;
      this.ctx.lineCap = "round";
      this.ctx.lineJoin = "round";
      
      // Use quadratic bezier for slight curve
      this.ctx.moveTo(startX, startY);
      this.ctx.quadraticCurveTo(midX, midY, endX, endY);
      this.ctx.stroke();
    });
    
    // Draw joints with gradient for better visibility
    landmarks.forEach((landmark, index) => {
      if (!landmark) return;
      if (landmark.visibility && landmark.visibility < this.options.minVisibility!) return;

      const x = landmark.x * width;
      const y = landmark.y * height;
      const radius = this.options.jointRadius!;

      // Create gradient for joint
      const gradient = this.ctx.createRadialGradient(x, y, 0, x, y, radius);
      gradient.addColorStop(0, this.options.jointColor!);
      gradient.addColorStop(0.7, this.options.jointColor! + "dd");
      gradient.addColorStop(1, this.options.jointColor! + "88");

      this.ctx.beginPath();
      this.ctx.arc(x, y, radius, 0, 2 * Math.PI);
      this.ctx.fillStyle = gradient;
      this.ctx.fill();
      
      // Add subtle outline
      this.ctx.strokeStyle = this.options.jointColor! + "44";
      this.ctx.lineWidth = 1;
      this.ctx.stroke();

      // Highlight thumb and index tips (for pinch detection)
      if (index === 4 || index === 8) {
        this.ctx.beginPath();
        this.ctx.arc(x, y, radius + 2, 0, 2 * Math.PI);
        this.ctx.strokeStyle = this.options.pinchColor!;
        this.ctx.lineWidth = 2;
        this.ctx.stroke();
      }
    });
    
    this.ctx.restore();
  }

  /**
   * Render pinch indicator with smooth animation
   */
  renderPinch(
    pinch: PinchCoordinates,
    isPinching: boolean,
    handedness: string = "Unknown"
  ) {
    if (!pinch.normalizedMidX || !pinch.normalizedMidY) return;

    const { width, height } = this.canvas;
    const x = pinch.normalizedMidX * width;
    const y = pinch.normalizedMidY * height;

    this.ctx.save();
    
    // Pinch indicator with pulsing effect
    const baseRadius = this.options.pinchRadius!;
    const radius = isPinching ? baseRadius * 1.2 : baseRadius;
    
    // Outer glow
    if (isPinching) {
      const glowGradient = this.ctx.createRadialGradient(x, y, 0, x, y, radius * 2);
      glowGradient.addColorStop(0, this.options.pinchColor! + "44");
      glowGradient.addColorStop(1, this.options.pinchColor! + "00");
      
      this.ctx.beginPath();
      this.ctx.arc(x, y, radius * 2, 0, 2 * Math.PI);
      this.ctx.fillStyle = glowGradient;
      this.ctx.fill();
    }
    
    // Main pinch circle
    this.ctx.beginPath();
    this.ctx.arc(x, y, radius, 0, 2 * Math.PI);
    this.ctx.strokeStyle = isPinching ? this.options.pinchColor! : "#ffffff";
    this.ctx.lineWidth = 3;
    this.ctx.stroke();
    
    // Inner dot
    this.ctx.beginPath();
    this.ctx.arc(x, y, 3, 0, 2 * Math.PI);
    this.ctx.fillStyle = isPinching ? this.options.pinchColor! : "#ffffff";
    this.ctx.fill();
    
    // Coordinate text with better styling
    if (this.options.showPinchCoords) {
      const coordText = `${handedness} Pinch: ${Math.round(pinch.x)}, ${Math.round(pinch.y)}`;
      
      // Mirror the canvas for text
      this.ctx.save();
      this.ctx.scale(-1, 1);
      
      this.ctx.font = "bold 12px monospace";
      this.ctx.textAlign = "left";
      const textMetrics = this.ctx.measureText(coordText);
      const textWidth = textMetrics.width;
      const textHeight = 16;
      
      // Calculate text position
      const mirroredX = width - x;
      const textX = -(mirroredX - radius - 10);
      const textY = y;
      
      // Background with rounded corners
      const padding = 6;
      const bgX = textX - padding;
      const bgY = textY - textHeight;
      const bgWidth = textWidth + padding * 2;
      const bgHeight = textHeight + padding;
      const borderRadius = 4;
      
      // Draw rounded rectangle background
      this.ctx.beginPath();
      this.ctx.moveTo(bgX + borderRadius, bgY);
      this.ctx.lineTo(bgX + bgWidth - borderRadius, bgY);
      this.ctx.quadraticCurveTo(bgX + bgWidth, bgY, bgX + bgWidth, bgY + borderRadius);
      this.ctx.lineTo(bgX + bgWidth, bgY + bgHeight - borderRadius);
      this.ctx.quadraticCurveTo(bgX + bgWidth, bgY + bgHeight, bgX + bgWidth - borderRadius, bgY + bgHeight);
      this.ctx.lineTo(bgX + borderRadius, bgY + bgHeight);
      this.ctx.quadraticCurveTo(bgX, bgY + bgHeight, bgX, bgY + bgHeight - borderRadius);
      this.ctx.lineTo(bgX, bgY + borderRadius);
      this.ctx.quadraticCurveTo(bgX, bgY, bgX + borderRadius, bgY);
      this.ctx.closePath();
      
      this.ctx.fillStyle = "rgba(0, 0, 0, 0.8)";
      this.ctx.fill();
      
      // Draw text
      this.ctx.fillStyle = "#ffffff";
      this.ctx.fillText(coordText, textX, textY - 2);
      
      this.ctx.restore();
    }
    
    this.ctx.restore();
  }

  /**
   * Update render options
   */
  updateOptions(options: Partial<RenderOptions>) {
    this.options = { ...this.options, ...options };
  }
}