import React, { createContext, useContext, useState, useCallback } from "react";

type ToastType = "success" | "error" | "info";
interface Toast {
  id: number;
  type: ToastType;
  message: string;
  detail?: string;
}

const WalletToastContext = createContext<{
  showToast: (type: ToastType, message: string, detail?: string) => void;
}>({ showToast: () => {} });

let toastId = 0;

export const WalletToastProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const showToast = useCallback((type: ToastType, message: string, detail?: string) => {
    const id = ++toastId;
    setToasts((t) => [...t, { id, type, message, detail }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 4000);
  }, []);
  return (
    <WalletToastContext.Provider value={{ showToast }}>
      {children}
      <div className="fixed left-4 right-4 top-4 z-[9999] flex flex-col gap-2 pointer-events-none">
        {toasts.map((t) => (
          <div
            key={t.id}
            className={`
              rounded-lg border px-4 py-3 text-sm shadow-lg
              ${t.type === "success" ? "bg-chart-2/90 text-primary-foreground border-chart-2" : ""}
              ${t.type === "error" ? "bg-destructive/90 text-white border-destructive" : ""}
              ${t.type === "info" ? "bg-primary text-primary-foreground border-primary" : ""}
            `}
          >
            {t.message}
            {t.detail && <span className="block text-xs opacity-90 mt-1">{t.detail}</span>}
          </div>
        ))}
      </div>
    </WalletToastContext.Provider>
  );
};

export const useWalletToast = () => useContext(WalletToastContext);
