<x-layouts.app>
    <div class="relative h-full overflow-hidden bg-background">
        <div id="main-content" class="relative z-10 flex flex-col items-center justify-center min-h-screen p-4">
            <div class="flex h-full w-full flex-col items-center justify-center text-zinc-200">
                <div class="h-full w-full lg:py-[18px]">
                    <div class="m-auto text-base px-3 md:px-4 w-full md:px-5 lg:px-4 xl:px-5 h-full">
                        <div class="mx-auto flex h-full w-full flex-col text-base justify-between md:max-w-3xl">
                            <div id="message-list" class="flex-grow overflow-y-auto space-y-4">
                                @if(isset($thread) && $thread->messages->isNotEmpty())
                                    @include('chat.show', ['messages' => $thread->messages])
                                @else
                                    <div class="mb-7 text-center">
                                        <div class="select-none pointer-events-none inline-flex justify-center text-2xl font-semibold leading-9">
                                            <h1>How can we help?</h1>
                                        </div>
                                    </div>
                                @endif
                            </div>
                            <div class="mt-4">
                                @include('dashboard.message-form', ['thread' => $thread ?? null])
                                @include('dashboard.terms-privacy')
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
</x-layouts.app>