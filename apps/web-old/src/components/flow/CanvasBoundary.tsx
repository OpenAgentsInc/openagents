import { Component } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import { TreeElementNode } from './TreeElementNode';

type ErrorBoundaryProps = {
  fallback: (error: Error, reset: () => void) => ReactNode;
  children: ReactNode;
};

type ErrorBoundaryState = {
  error: Error | null;
};

class ErrorBoundaryClass extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    console.error('Flow canvas error:', error, errorInfo);
  }

  reset = (): void => {
    this.setState({ error: null });
  };

  render(): ReactNode {
    if (this.state.error) {
      return this.props.fallback(this.state.error, this.reset);
    }
    return this.props.children;
  }
}

type CanvasBoundaryProps = {
  children: ReactNode;
};

export function CanvasBoundary({ children }: CanvasBoundaryProps) {
  return (
    <ErrorBoundaryClass
      fallback={(error, reset) => (
        <TreeElementNode id="flow-error-state" position={{ x: 0, y: 250 }}>
          <div className="w-[400px]">
            <div className="relative rounded-xl border border-border bg-card shadow-sm overflow-hidden">
              <div className="flex h-12 items-center gap-3 border-b border-border px-4">
                <span className="text-sm font-medium text-foreground">
                  Failed to render network tree
                </span>
              </div>
              <div className="space-y-2 p-4">
                <p className="text-xs font-medium text-muted-foreground">{error.message}</p>
                <p className="text-xs text-muted-foreground">Check console for details</p>
              </div>
              <div className="p-4 pt-0">
                <Button variant="outline" size="sm" className="w-full" onClick={reset}>
                  Retry Layout
                </Button>
              </div>
            </div>
          </div>
        </TreeElementNode>
      )}
    >
      {children}
    </ErrorBoundaryClass>
  );
}
