<x-layouts.app>
    <div class="relative h-full overflow-hidden bg-background">
        <div id="main-content" class="relative z-10 flex flex-col items-center justify-center min-h-screen p-4">
            <div class="flex h-full w-full flex-col items-center justify-center text-zinc-200">
                <div class="h-full w-full lg:py-[18px]">
                    <div class="m-auto text-base px-3 md:px-4 w-full md:px-5 lg:px-4 xl:px-5 h-full">
                        <div class="mx-auto flex h-full w-full flex-col text-base justify-between md:max-w-3xl">
                            <div id="message-list" class="flex-grow overflow-y-auto space-y-4">
                                @if(isset($thread) && $thread->messages->isNotEmpty())
                                @include('threads.show', ['messages' => $thread->messages])
                                @else
                                <div class="mb-7 text-center">
                                    <div class="select-none pointer-events-none inline-flex justify-center text-2xl font-semibold leading-9">
                                        <h1>How can we help?</h1>
                                    </div>
                                </div>
                                @endif
                            </div>
                            <div class="mt-4">
                                <x-dashboard.message-form :thread="$thread ?? null" />
                                <x-dashboard.terms-privacy />
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        <div id="teams-and-projects" hx-get="{{ route('teams.projects') }}" hx-trigger="load"></div>
    </div>
</x-layouts.app>

<script>
    console.log('Dashboard script loaded');

    function focusTextarea() {
        console.log('Attempting to focus textarea');
        const textarea = document.getElementById('message-textarea');
        if (textarea) {
            console.log('Textarea found, focusing');
            textarea.focus();
        } else {
            console.log('Textarea not found');
        }
    }

    document.addEventListener('DOMContentLoaded', function() {
        console.log('DOM fully loaded');

        // Set up MutationObserver to watch for changes in the DOM
        const observer = new MutationObserver(function(mutations) {
            mutations.forEach(function(mutation) {
                if (mutation.type === 'childList') {
                    const addedNodes = mutation.addedNodes;
                    for (let i = 0; i < addedNodes.length; i++) {
                        if (addedNodes[i].id === 'message-textarea') {
                            console.log('Textarea added to DOM');
                            focusTextarea();
                            observer.disconnect(); // Stop observing once we've found the textarea
                            break;
                        }
                    }
                }
            });
        });

        // Start observing the document with the configured parameters
        observer.observe(document.body, { childList: true, subtree: true });

        document.body.addEventListener('chatLoaded', function() {
            console.log('chatLoaded event received');
            focusTextarea();
        });
    });

    // Immediate execution
    (function() {
        console.log('Immediate function executed');
        focusTextarea();
    })();
</script>