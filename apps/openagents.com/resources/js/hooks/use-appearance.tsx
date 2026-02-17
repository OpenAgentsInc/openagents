import { useCallback, useMemo } from 'react';

export type ResolvedAppearance = 'light' | 'dark';
export type Appearance = ResolvedAppearance | 'system';

export type UseAppearanceReturn = {
    readonly appearance: Appearance;
    readonly resolvedAppearance: ResolvedAppearance;
    readonly updateAppearance: (mode: Appearance) => void;
};

function applyDarkTheme(): void {
    if (typeof document === 'undefined') return;
    document.documentElement.classList.add('dark');
    document.documentElement.style.colorScheme = 'dark';
}

export function initializeTheme(): void {
    if (typeof window === 'undefined') return;
    applyDarkTheme();
}

export function useAppearance(): UseAppearanceReturn {
    const appearance: Appearance = 'dark';
    const resolvedAppearance: ResolvedAppearance = 'dark';
    const updateAppearance = useCallback((_mode: Appearance): void => {
        // No-op: app is always dark
    }, []);

    return useMemo(
        () => ({ appearance, resolvedAppearance, updateAppearance }),
        [updateAppearance],
    );
}
