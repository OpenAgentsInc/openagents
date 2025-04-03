import React from "react";

export default function DragWindowRegion() {
  return (
    <div className="fixed top-0 left-0 right-0 z-50 border-b">
      <div className="flex w-screen items-stretch justify-between">
        <div className="draglayer w-full h-[30px]">
        </div>
      </div>
    </div>
  );
}
