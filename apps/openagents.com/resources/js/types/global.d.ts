import type { Auth } from '@/types/auth';

type SharedChatThread = {
    id: string;
    title: string;
    updatedAt: string | null;
};

declare module '@inertiajs/core' {
    export interface InertiaConfig {
        sharedPageProps: {
            name: string;
            auth: Auth;
            sidebarOpen: boolean;
            isAdmin: boolean;
            chatThreads: SharedChatThread[];
            [key: string]: unknown;
        };
    }
}
