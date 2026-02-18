import { Head } from '@inertiajs/react';
import Heading from '@/components/heading';
import SettingsLayout from '@/layouts/settings/layout';

export default function Appearance() {
    return (
        <>
            <Head title="Profile settings" />

            <SettingsLayout>
                <div className="space-y-6">
                    <Heading
                        variant="small"
                        title="Profile settings"
                        description="Appearance settings were removed."
                    />
                </div>
            </SettingsLayout>
        </>
    );
}
