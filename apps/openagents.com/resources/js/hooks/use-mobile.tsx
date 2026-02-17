import { useSyncExternalStore } from 'react';

const MOBILE_BREAKPOINT = 768;
const MEDIA_QUERY = `(max-width: ${MOBILE_BREAKPOINT - 1}px)`;

function getSnapshot(): boolean {
    if (typeof window === 'undefined') {
        return false;
    }

    return window.matchMedia(MEDIA_QUERY).matches;
}

function getServerSnapshot(): boolean {
    return false;
}

function subscribe(onStoreChange: () => void): () => void {
    if (typeof window === 'undefined') {
        return () => {};
    }

    const mql = window.matchMedia(MEDIA_QUERY);
    const handler = () => onStoreChange();

    if (typeof mql.addEventListener === 'function') {
        mql.addEventListener('change', handler);

        return () => {
            mql.removeEventListener('change', handler);
        };
    }

    mql.addListener(handler);

    return () => {
        mql.removeListener(handler);
    };
}

export function useIsMobile(): boolean {
    return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
