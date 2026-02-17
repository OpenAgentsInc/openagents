import { Head } from '@inertiajs/react';

export default function Index() {
    return (
        <>
            <Head title="" />
            <div
                className="fixed inset-0 bg-black"
                style={{ width: '100vw', height: '100vh' }}
            >
                <div className="min-h-pwa relative flex w-full flex-1 flex-col overflow-y-clip transition-[width,height] print:absolute print:top-0 print:left-0 print:h-auto print:min-h-auto print:overflow-visible">
                </div>
            </div>
        </>
    );
}
