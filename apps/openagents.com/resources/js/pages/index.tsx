import { Head } from '@inertiajs/react';
import { ArrowUpIcon } from '@radix-ui/react-icons';
import { useCallback, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

/**
 * Submit behavior adapted from assistant-ui:
 * - ComposerInput: Enter submits, Shift+Enter newline, IME composition respected, requestSubmit() from keydown
 * - ComposerRoot: preventDefault then call send
 * - with-cloud-standalone Composer: only submit when value.trim() && !isRunning, disable send when empty
 */
export default function Index() {
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const [value, setValue] = useState('');
    const [isComposing, setIsComposing] = useState(false);

    const handleKeyDown = useCallback(
        (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
            if (e.key !== 'Enter') return;
            if (isComposing || e.nativeEvent.isComposing) return;
            if (e.shiftKey) return; // Shift+Enter = newline (assistant-ui submitMode "enter")
            e.preventDefault();
            if (!value.trim()) return;
            const form = e.currentTarget.form;
            const submitButton = form?.querySelector<HTMLButtonElement>('button[type="submit"]');
            if (submitButton?.disabled) return;
            form?.requestSubmit();
        },
        [isComposing, value],
    );

    const handleSubmit = useCallback((e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        const trimmed = value.trim();
        if (!trimmed) return;
        console.log('submit', trimmed);
        setValue('');
        // Keep cursor in input (assistant-ui ComposerInput focus pattern)
        requestAnimationFrame(() => {
            textareaRef.current?.focus({ preventScroll: true });
            textareaRef.current?.setSelectionRange(0, 0);
        });
    }, [value]);

    return (
        <>
            <Head title="" />
            <div className="chat-page-root min-h-pwa flex h-dvh flex-col overflow-hidden bg-black">
                <main className="firefox-scrollbar-margin-fix relative flex min-h-0 w-full flex-1 flex-col overflow-hidden transition-[width,height] print:absolute print:left-0 print:top-0 print:h-auto print:min-h-auto print:overflow-visible">
                    {/* Scrollable content area */}
                    <div
                        className="absolute inset-0 overflow-y-auto print:static print:inset-auto print:block print:h-auto print:overflow-visible print:pb-0"
                        style={{ paddingBottom: 144, scrollbarGutter: 'stable both-edges' }}
                    >
                        <div className="mx-auto flex min-h-[calc(100vh-20rem)] w-full max-w-3xl flex-col px-4 pt-8 pb-10" />
                    </div>

                    {/* Fixed bottom input bar */}
                    <div className="pointer-events-none absolute bottom-0 z-10 w-full overflow-x-visible px-2">
                        <div className="relative mx-auto flex w-full max-w-3xl flex-col overflow-x-visible text-center">
                            <div className="pointer-events-auto mx-auto w-full max-w-[calc(100%-2rem)]">
                                <div
                                    className={cn(
                                        'chat-input-outer min-w-0 overflow-hidden rounded-t-lg border border-b-0 border-white/20 p-2 pb-0 backdrop-blur-lg',
                                        'dark:border-white/12',
                                    )}
                                >
                                    <form
                                        className={cn(
                                            'chat-input-form pointer-events-auto relative flex w-full min-w-0 flex-col items-stretch gap-2 rounded-t-md border border-b-0 border-white/25 px-3 pt-3 pb-3 text-secondary-foreground outline-none',
                                            'max-sm:pb-6 sm:max-w-3xl dark:border-white/15',
                                        )}
                                        id="chat-input-form"
                                        onSubmit={handleSubmit}
                                    >
                                        <div className="flex min-w-0 grow flex-row items-start">
                                            <textarea
                                                ref={textareaRef}
                                                name="input"
                                                id="chat-input"
                                                placeholder="Type message here"
                                                value={value}
                                                onChange={(e) => setValue(e.target.value)}
                                                className="chat-input-textarea w-full min-w-0 resize-none bg-transparent text-base leading-6 text-foreground outline-none placeholder:text-muted-foreground/60 disabled:opacity-50"
                                                aria-label="Message input"
                                                autoComplete="off"
                                                autoFocus
                                                rows={1}
                                                onKeyDown={handleKeyDown}
                                                onCompositionStart={() => setIsComposing(true)}
                                                onCompositionEnd={() => setIsComposing(false)}
                                            />
                                        </div>
                                        <div className="@container -mb-px mt-2 flex w-full min-w-0 flex-row-reverse justify-between">
                                            <div className="-mr-0.5 -mt-0.5 flex shrink-0 items-center justify-center gap-2">
                                                <Button
                                                    type="submit"
                                                    size="icon"
                                                    className="size-9 rounded-lg"
                                                    aria-label="Send message"
                                                    disabled={!value.trim()}
                                                >
                                                    <ArrowUpIcon className="size-5" />
                                                </Button>
                                            </div>
                                        </div>
                                    </form>
                                </div>
                            </div>
                        </div>
                    </div>
                </main>
            </div>
        </>
    );
}
