/**
 * Settings top bar – 40 px tall, stays above sidebar overlay.
 */
export function HeaderSettings() {
  return (
    <header className="select-none w-full h-10 border-b bg-background flex-shrink-0 relative z-40">
      <div className="h-full w-full flex items-center px-4">
        <div className="flex items-center">
          <h2 className="text-sm font-medium">Settings</h2>
        </div>
        <div className="ml-auto flex items-center gap-2">
          {/* Add any settings-specific actions here */}
        </div>
      </div>
    </header>
  );
}
