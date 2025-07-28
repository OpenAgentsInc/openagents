import React, { useState, useEffect, useRef, ReactNode } from "react";
import { useDrag } from "@use-gesture/react";
import { X as CloseIcon } from "lucide-react";
import { Pane as PaneType } from "@/types/pane";
import { usePaneStore } from "@/stores/pane";
import type { FullGestureState } from "@use-gesture/react";
import { cn } from "@/lib/utils";

type PaneProps = PaneType & {
  children?: ReactNode;
  style?: React.CSSProperties;
};

type ResizeCorner =
  | "topleft"
  | "top"
  | "topright"
  | "right"
  | "bottomright"
  | "bottom"
  | "bottomleft"
  | "left";

const useResizeHandlers = (
  id: string,
  initialPosition: { x: number; y: number },
  initialSize: { width: number; height: number },
  updatePanePosition: (id: string, x: number, y: number) => void,
  updatePaneSize: (id: string, width: number, height: number) => void,
  isCurrentlyInteracting: boolean,
  setIsResizing: (isResizing: boolean) => void,
) => {
  const [position, setPosition] = useState(initialPosition);
  const [size, setSize] = useState(initialSize);

  const prevPositionRef = useRef(initialPosition);
  const prevSizeRef = useRef(initialSize);

  useEffect(() => {
    if (
      !isCurrentlyInteracting &&
      (initialPosition.x !== prevPositionRef.current.x ||
        initialPosition.y !== prevPositionRef.current.y) &&
      (position.x !== initialPosition.x || position.y !== initialPosition.y)
    ) {
      setPosition(initialPosition);
    }
    if (!isCurrentlyInteracting) {
      prevPositionRef.current = initialPosition;
    }
  }, [
    id,
    initialPosition.x,
    initialPosition.y,
    isCurrentlyInteracting,
    position.x,
    position.y,
  ]);

  useEffect(() => {
    if (
      !isCurrentlyInteracting &&
      (initialSize.width !== prevSizeRef.current.width ||
        initialSize.height !== prevSizeRef.current.height) &&
      (size.width !== initialSize.width || size.height !== initialSize.height)
    ) {
      setSize(initialSize);
    }
    if (!isCurrentlyInteracting) {
      prevSizeRef.current = initialSize;
    }
  }, [
    initialSize.width,
    initialSize.height,
    isCurrentlyInteracting,
    size.width,
    size.height,
  ]);

  const minWidth = 200;
  const minHeight = 100;

  const resizeStartRef = useRef({ x: 0, y: 0, width: 0, height: 0 });

  const createResizeHandler = (corner: ResizeCorner) => {
    return useDrag(
      (state: FullGestureState<"drag">) => {
        const { first, movement: [mx, my] } = state;

        if (first) {
          resizeStartRef.current = {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
          };
          setIsResizing(true);
        }

        let newWidth = resizeStartRef.current.width;
        let newHeight = resizeStartRef.current.height;
        let newX = resizeStartRef.current.x;
        let newY = resizeStartRef.current.y;

        switch (corner) {
          case "topleft":
            newWidth = Math.max(minWidth, resizeStartRef.current.width - mx);
            newHeight = Math.max(minHeight, resizeStartRef.current.height - my);
            newX = resizeStartRef.current.x + (resizeStartRef.current.width - newWidth);
            newY = resizeStartRef.current.y + (resizeStartRef.current.height - newHeight);
            break;
          case "top":
            newHeight = Math.max(minHeight, resizeStartRef.current.height - my);
            newY = resizeStartRef.current.y + (resizeStartRef.current.height - newHeight);
            break;
          case "topright":
            newWidth = Math.max(minWidth, resizeStartRef.current.width + mx);
            newHeight = Math.max(minHeight, resizeStartRef.current.height - my);
            newY = resizeStartRef.current.y + (resizeStartRef.current.height - newHeight);
            break;
          case "right":
            newWidth = Math.max(minWidth, resizeStartRef.current.width + mx);
            break;
          case "bottomright":
            newWidth = Math.max(minWidth, resizeStartRef.current.width + mx);
            newHeight = Math.max(minHeight, resizeStartRef.current.height + my);
            break;
          case "bottom":
            newHeight = Math.max(minHeight, resizeStartRef.current.height + my);
            break;
          case "bottomleft":
            newWidth = Math.max(minWidth, resizeStartRef.current.width - mx);
            newHeight = Math.max(minHeight, resizeStartRef.current.height + my);
            newX = resizeStartRef.current.x + (resizeStartRef.current.width - newWidth);
            break;
          case "left":
            newWidth = Math.max(minWidth, resizeStartRef.current.width - mx);
            newX = resizeStartRef.current.x + (resizeStartRef.current.width - newWidth);
            break;
        }

        setPosition({ x: newX, y: newY });
        setSize({ width: newWidth, height: newHeight });

        if (state.last) {
          updatePanePosition(id, newX, newY);
          updatePaneSize(id, newWidth, newHeight);
          setIsResizing(false);
        }
      },
      { from: [0, 0] },
    );
  };

  return {
    position,
    size,
    resizeHandlers: {
      topleft: createResizeHandler("topleft"),
      top: createResizeHandler("top"),
      topright: createResizeHandler("topright"),
      right: createResizeHandler("right"),
      bottomright: createResizeHandler("bottomright"),
      bottom: createResizeHandler("bottom"),
      bottomleft: createResizeHandler("bottomleft"),
      left: createResizeHandler("left"),
    },
  };
};

export const Pane: React.FC<PaneProps> = ({
  id,
  title,
  x,
  y,
  width,
  height,
  type: _type,
  isActive,
  dismissable = true,
  children,
  style,
}) => {
  const { 
    updatePanePosition, 
    updatePaneSize, 
    removePane, 
    bringPaneToFront
  } = usePaneStore();
  
  const [isDragging, setIsDragging] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [dragPosition, setDragPosition] = useState({ x, y });
  
  // Sync position from props when not interacting
  useEffect(() => {
    if (!isDragging && !isResizing) {
      setDragPosition({ x, y });
    }
  }, [x, y, isDragging, isResizing]);

  const isCurrentlyInteracting = isDragging || isResizing;

  const {
    position,
    size,
    resizeHandlers,
  } = useResizeHandlers(
    id,
    { x, y },
    { width, height },
    updatePanePosition,
    updatePaneSize,
    isCurrentlyInteracting,
    setIsResizing,
  );

  // Commander's exact bounds calculation
  const screenWidth = typeof window !== "undefined" ? window.innerWidth : 1920;
  const screenHeight = typeof window !== "undefined" ? window.innerHeight : 1080;
  const bounds = {
    left: 20, // PANE_MARGIN
    top: 20,  // PANE_MARGIN
    right: screenWidth - width - 20,  // PANE_MARGIN
    bottom: screenHeight - height - 20, // PANE_MARGIN
  };

  const bindDrag = useDrag(
    (state: FullGestureState<"drag">) => {
      const { first, active, last, memo, xy: [pointerX, pointerY] } = state;

      if (first) {
        const initialMemo = {
          startX: pointerX,
          startY: pointerY,
          paneX: position.x, // Use store position, not dragPosition
          paneY: position.y,
        };

        // Only activate if not already active (Commander pattern)
        if (!isActive) {
          bringPaneToFront(id);
        }
        
        setIsDragging(true);
        return initialMemo;
      }

      if (memo && (active || last)) {
        const deltaX = pointerX - memo.startX;
        const deltaY = pointerY - memo.startY;

        let newX = memo.paneX + deltaX;
        let newY = memo.paneY + deltaY;

        // Commander's exact bounds constraints
        newX = Math.max(bounds.left, Math.min(newX, bounds.right));
        newY = Math.max(bounds.top, Math.min(newY, bounds.bottom));

        // Update local position during drag
        if (active) {
          setDragPosition({ x: newX, y: newY });
        }

        if (last) {
          updatePanePosition(id, newX, newY);
          setIsDragging(false);
        }
      }

      return memo;
    },
    { filterTaps: true }
  );

  const handleClose = () => {
    removePane(id);
  };

  const handleMouseDown = () => {
    // Only bring to front if not already active (Commander pattern)
    if (!isActive) {
      bringPaneToFront(id);
    }
  };

  return (
    <div
      className={cn(
        "absolute chat-pane shadow-xl",
        "transition-all duration-200 ease-out",
        isActive && "ring-2 ring-primary/20",
        isCurrentlyInteracting && "transition-none"
      )}
      style={{
        border: isActive ? "1px solid var(--primary)" : "1px solid #444",
        left: `${isDragging ? dragPosition.x : position.x}px`,
        top: `${isDragging ? dragPosition.y : position.y}px`,
        width: `${isResizing ? size.width : width}px`,
        height: `${isResizing ? size.height : height}px`,
        ...style,
      }}
      onMouseDown={handleMouseDown}
    >
      {/* Corner Brackets - only show when not active */}
      {!isActive && (
        <>
          <div className="corner-bracket-top-left" />
          <div className="corner-bracket-top-right" />
          <div className="corner-bracket-bottom-left" />
          <div className="corner-bracket-bottom-right" />
        </>
      )}
      {/* Title Bar */}
      <div
        {...bindDrag()}
        className="pane-header flex items-center justify-between cursor-move select-none"
      >
        <h3 className="text-sm font-semibold truncate flex-1">{title}</h3>
        {dismissable && (
          <button
            onClick={handleClose}
            className="ml-2 p-1 rounded hover:bg-destructive/10 hover:text-destructive transition-colors"
          >
            {/* @ts-ignore React 19 JSX component type issue */}
            <CloseIcon className="h-4 w-4" />
          </button>
        )}
      </div>

      {/* Content */}
      <div className="p-4 h-[calc(100%-40px)] overflow-auto">
        {children}
      </div>

      {/* Resize Handles */}
      {/* Corners */}
      <div
        {...resizeHandlers.topleft()}
        className="absolute -top-1 -left-1 w-3 h-3 cursor-nw-resize"
      />
      <div
        {...resizeHandlers.topright()}
        className="absolute -top-1 -right-1 w-3 h-3 cursor-ne-resize"
      />
      <div
        {...resizeHandlers.bottomleft()}
        className="absolute -bottom-1 -left-1 w-3 h-3 cursor-sw-resize"
      />
      <div
        {...resizeHandlers.bottomright()}
        className="absolute -bottom-1 -right-1 w-3 h-3 cursor-se-resize"
      />

      {/* Edges */}
      <div
        {...resizeHandlers.top()}
        className="absolute -top-1 left-3 right-3 h-3 cursor-n-resize"
      />
      <div
        {...resizeHandlers.bottom()}
        className="absolute -bottom-1 left-3 right-3 h-3 cursor-s-resize"
      />
      <div
        {...resizeHandlers.left()}
        className="absolute top-3 -left-1 bottom-3 w-3 cursor-w-resize"
      />
      <div
        {...resizeHandlers.right()}
        className="absolute top-3 -right-1 bottom-3 w-3 cursor-e-resize"
      />
    </div>
  );
};