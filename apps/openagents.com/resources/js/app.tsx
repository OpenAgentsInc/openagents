import { createInertiaApp } from '@inertiajs/react';
import { PostHogProvider } from '@posthog/react';
import { resolvePageComponent } from 'laravel-vite-plugin/inertia-helpers';
import React, { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { PostHogIdentify } from '@/components/posthog-identify';
import { TooltipProvider } from '@/components/ui/tooltip';
import { GlobalSidebarLayout } from '@/layouts/global-sidebar-layout';
import { posthog } from '@/lib/posthog';
import '../css/app.css';
import { initializeTheme } from './hooks/use-appearance';

const appName = import.meta.env.VITE_APP_NAME || 'OpenAgents';

createInertiaApp({
    title: (title) =>
        title ? (title === appName ? title : `${title} - ${appName}`) : appName,
    resolve: async (name) => {
        const module = (await resolvePageComponent(
            `./pages/${name}.tsx`,
            import.meta.glob('./pages/**/*.tsx'),
        )) as { default: React.ComponentType<object> };
        const Page = module.default;
        return function InertiaPageWrapper(props: object) {
            return (
                <>
                    <PostHogIdentify />
                    <Page {...props} />
                </>
            );
        };
    },
    setup({ el, App, props }) {
        const root = createRoot(el);

        root.render(
            <StrictMode>
                <PostHogProvider client={posthog}>
                        <TooltipProvider>
                        <GlobalSidebarLayout>
                            <App {...props} />
                        </GlobalSidebarLayout>
                    </TooltipProvider>
                </PostHogProvider>
            </StrictMode>,
        );
    },
    progress: {
        color: '#4B5563',
    },
});

// This will set light / dark mode on load...
initializeTheme();
