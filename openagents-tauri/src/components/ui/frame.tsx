import React from 'react';
import { cn } from '@/lib/utils';

interface FrameProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
  cornerLength?: number;
  showCorners?: boolean;
}

const Frame = React.forwardRef<HTMLDivElement, FrameProps>(
  ({ className, children, cornerLength = 20, showCorners = true, ...props }, ref) => {
    return (
      <div 
        ref={ref}
        className={cn(
          "relative bg-card border border-border backdrop-blur-sm",
          "transition-all duration-200 ease-out",
          "hover:border-primary/40 hover:shadow-lg hover:shadow-primary/20",
          className
        )}
        {...props}
      >
        {showCorners && (
          <>
            {/* Top Left Corner */}
            <div 
              className="absolute -top-0.5 -left-0.5 border-l-2 border-t-2 border-primary/80 pointer-events-none z-20"
              style={{ 
                width: `${cornerLength}px`, 
                height: `${cornerLength}px`,
                filter: 'drop-shadow(0 0 6px rgb(255 255 255 / 0.1))',
                transition: 'all 0.2s ease-out'
              }}
            />
            
            {/* Top Right Corner */}
            <div 
              className="absolute -top-0.5 -right-0.5 border-r-2 border-t-2 border-primary/80 pointer-events-none z-20"
              style={{ 
                width: `${cornerLength}px`, 
                height: `${cornerLength}px`,
                filter: 'drop-shadow(0 0 6px rgb(255 255 255 / 0.1))',
                transition: 'all 0.2s ease-out'
              }}
            />
            
            {/* Bottom Left Corner */}
            <div 
              className="absolute -bottom-0.5 -left-0.5 border-l-2 border-b-2 border-primary/80 pointer-events-none z-20"
              style={{ 
                width: `${cornerLength}px`, 
                height: `${cornerLength}px`,
                filter: 'drop-shadow(0 0 6px rgb(255 255 255 / 0.1))',
                transition: 'all 0.2s ease-out'
              }}
            />
            
            {/* Bottom Right Corner */}
            <div 
              className="absolute -bottom-0.5 -right-0.5 border-r-2 border-b-2 border-primary/80 pointer-events-none z-20"
              style={{ 
                width: `${cornerLength}px`, 
                height: `${cornerLength}px`,
                filter: 'drop-shadow(0 0 6px rgb(255 255 255 / 0.1))',
                transition: 'all 0.2s ease-out'
              }}
            />
          </>
        )}
        
        {children}
      </div>
    );
  }
);

Frame.displayName = "Frame";

export { Frame };