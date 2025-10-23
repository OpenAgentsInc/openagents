import React, { createContext, useCallback, useContext, useMemo, useState } from 'react';

type DrawerControls = {
  open: boolean;
  setOpen: (v: boolean) => void;
  toggle: () => void;
};

const Ctx = createContext<DrawerControls | undefined>(undefined);

export function DrawerProvider({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  const toggle = useCallback(() => setOpen((v) => !v), []);
  const value = useMemo(() => ({ open, setOpen, toggle }), [open, toggle]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useDrawer(): DrawerControls {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error('useDrawer must be used within DrawerProvider');
  return ctx;
}

