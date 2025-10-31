import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { useDrawerStore } from '@/lib/drawer-store'

type DrawerControls = {
  open: boolean;
  setOpen: (v: boolean) => void;
  toggle: () => void;
};

const Ctx = createContext<DrawerControls | undefined>(undefined);

export function DrawerProvider({ children }: { children: React.ReactNode }) {
  const persistedOpen = useDrawerStore((s) => s.open)
  const setPersistedOpen = useDrawerStore((s) => s.setOpen)
  const [open, setOpen] = useState<boolean>(persistedOpen);
  useEffect(() => { setOpen(persistedOpen) }, [persistedOpen])
  const toggle = useCallback(() => {
    setOpen((v) => {
      const nv = !v
      try { setPersistedOpen(nv) } catch {}
      return nv
    })
  }, [setPersistedOpen]);
  const setOpenBoth = useCallback((v: boolean) => {
    try { setPersistedOpen(v) } catch {}
    setOpen(v)
  }, [setPersistedOpen])
  const value = useMemo(() => ({ open, setOpen: setOpenBoth, toggle }), [open, setOpenBoth, toggle]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useDrawer(): DrawerControls {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error('useDrawer must be used within DrawerProvider');
  return ctx;
}
