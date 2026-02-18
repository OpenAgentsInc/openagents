import { Transition } from '@headlessui/react';
import { Form, Head, usePage } from '@inertiajs/react';
import { useEffect } from 'react';
import Heading from '@/components/heading';
import InputError from '@/components/input-error';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { usePostHogEvent } from '@/hooks/use-posthog-event';
import SettingsLayout from '@/layouts/settings/layout';

type PageProps = {
    auth: {
        user: {
            name: string;
            email?: string;
        };
    };
    status?: string | null;
    autopilotSettings?: {
        id: string;
        handle: string;
        displayName: string;
        tagline: string | null;
        configVersion: number;
        profile?: {
            ownerDisplayName?: string | null;
            personaSummary?: string | null;
            autopilotVoice?: string | null;
            principles?: string[] | null;
        } | null;
    } | null;
};

export default function AutopilotSettings() {
    const { auth, status, autopilotSettings } = usePage<PageProps>().props;
    const capture = usePostHogEvent('settings_autopilot');

    const principlesText = Array.isArray(autopilotSettings?.profile?.principles)
        ? autopilotSettings.profile.principles.join('\n')
        : '';

    useEffect(() => {
        capture('settings_autopilot.page_opened', {
            hasAutopilotSettings: Boolean(autopilotSettings),
            configVersion: autopilotSettings?.configVersion ?? null,
            userEmail: auth.user.email ?? null,
        });
    }, [auth.user.email, autopilotSettings, capture]);

    return (
        <>
            <Head title="Autopilot settings" />

            <h1 className="sr-only">Autopilot Settings</h1>

            <SettingsLayout>
                <div className="space-y-6">
                    <Heading
                        variant="small"
                        title="Autopilot profile"
                        description="Edit your persistent Autopilot identity and behavior preferences."
                    />

                    <Form
                        action="/settings/autopilot"
                        method="patch"
                        options={{ preserveScroll: true }}
                        className="space-y-6"
                        onSubmit={() => {
                            capture('settings_autopilot.save_submitted', {
                                userEmail: auth.user.email ?? null,
                                configVersion: autopilotSettings?.configVersion ?? null,
                            });
                        }}
                    >
                        {({ processing, recentlySuccessful, errors }) => (
                            <>
                                <div className="grid gap-2">
                                    <Label htmlFor="autopilot-display-name">Autopilot display name</Label>
                                    <Input
                                        id="autopilot-display-name"
                                        name="displayName"
                                        defaultValue={autopilotSettings?.displayName ?? `${auth.user.name} Autopilot`}
                                        placeholder="Autopilot"
                                    />
                                    <InputError className="mt-2" message={errors.displayName} />
                                </div>

                                <div className="grid gap-2">
                                    <Label htmlFor="autopilot-owner-display-name">How Autopilot addresses you</Label>
                                    <Input
                                        id="autopilot-owner-display-name"
                                        name="ownerDisplayName"
                                        defaultValue={autopilotSettings?.profile?.ownerDisplayName ?? auth.user.name}
                                        placeholder="Your preferred name"
                                    />
                                    <InputError className="mt-2" message={errors.ownerDisplayName} />
                                </div>

                                <div className="grid gap-2">
                                    <Label htmlFor="autopilot-voice">Autopilot voice</Label>
                                    <Input
                                        id="autopilot-voice"
                                        name="autopilotVoice"
                                        defaultValue={autopilotSettings?.profile?.autopilotVoice ?? ''}
                                        placeholder="calm, direct, pragmatic"
                                    />
                                    <InputError className="mt-2" message={errors.autopilotVoice} />
                                </div>

                                <div className="grid gap-2">
                                    <Label htmlFor="autopilot-tagline">Tagline</Label>
                                    <Input
                                        id="autopilot-tagline"
                                        name="tagline"
                                        defaultValue={autopilotSettings?.tagline ?? ''}
                                        placeholder="Persistent, careful, action-oriented"
                                    />
                                    <InputError className="mt-2" message={errors.tagline} />
                                </div>

                                <div className="grid gap-2">
                                    <Label htmlFor="autopilot-persona-summary">Persona summary</Label>
                                    <Textarea
                                        id="autopilot-persona-summary"
                                        name="personaSummary"
                                        defaultValue={autopilotSettings?.profile?.personaSummary ?? ''}
                                        placeholder="Describe how Autopilot should think and communicate."
                                        rows={5}
                                    />
                                    <InputError className="mt-2" message={errors.personaSummary} />
                                </div>

                                <div className="grid gap-2">
                                    <Label htmlFor="autopilot-principles">Core principles (one per line)</Label>
                                    <Textarea
                                        id="autopilot-principles"
                                        name="principlesText"
                                        defaultValue={principlesText}
                                        placeholder="I prefer verification over guessing."
                                        rows={5}
                                    />
                                    <InputError className="mt-2" message={errors.principlesText} />
                                </div>

                                <div className="flex items-center gap-4">
                                    <Button disabled={processing}>Save Autopilot profile</Button>

                                    <Transition
                                        show={recentlySuccessful || status === 'autopilot-updated'}
                                        enter="transition ease-in-out"
                                        enterFrom="opacity-0"
                                        leave="transition ease-in-out"
                                        leaveTo="opacity-0"
                                    >
                                        <p className="text-sm text-zinc-600 dark:text-zinc-300">Autopilot updated</p>
                                    </Transition>
                                </div>
                            </>
                        )}
                    </Form>
                </div>
            </SettingsLayout>
        </>
    );
}
