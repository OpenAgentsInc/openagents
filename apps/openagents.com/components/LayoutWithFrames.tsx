"use client";

import React, { type ReactNode } from 'react';
import {
  FrameOctagon,
  styleFrameClipOctagon
} from '@arwes/react';
import { ChatSidebar } from './ChatSidebar';

interface LayoutWithFramesProps {
  children: ReactNode;
  showSidebar?: boolean;
}

export const LayoutWithFrames = (props: LayoutWithFramesProps): React.ReactElement => {
  const { children, showSidebar = false } = props;

  return (
    <div className="flex-1 flex overflow-hidden">
        {/* Sidebar */}
        {showSidebar && <ChatSidebar />}
        
        {/* Main Content */}
        <main className="flex-1 flex flex-col p-4">
          <div className="relative flex-1 flex flex-col">
            {/* Main Background Frame */}
            <div 
              className="absolute inset-0"
              style={{
                clipPath: styleFrameClipOctagon({ squareSize: 16 })
              }}
            >
              <FrameOctagon
                style={{
                  // @ts-expect-error CSS variables
                  '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.15)',
                  '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.5)'
                }}
                squareSize={16}
              />
            </div>
            
            <div className="relative flex-1 flex flex-col overflow-hidden">
              {children}
            </div>
          </div>
        </main>

    </div>
  );
};