"use client";

import React, { type ReactNode } from 'react';
import { ChatSidebar } from './ChatSidebar';

interface LayoutWithFramesProps {
  children: ReactNode;
  showSidebar?: boolean;
}

export const LayoutWithFrames = (props: LayoutWithFramesProps): React.ReactElement => {
  const { children, showSidebar = false } = props;

  return (
    <div className="flex-1 flex overflow-hidden bg-black">
        {/* Sidebar */}
        {showSidebar && <ChatSidebar />}
        
        {/* Main Content */}
        <main className="flex-1 flex flex-col overflow-hidden">
          {children}
        </main>

    </div>
  );
};