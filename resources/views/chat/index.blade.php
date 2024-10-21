<x-layouts.app>
    <div class="relative h-screen overflow-hidden bg-background">
        <div id="main-content" class="relative z-10 flex flex-col h-screen">
            <div class="flex-grow overflow-y-auto">
                <div class="mx-auto max-w-3xl px-4 py-8">
                    <div id="message-list" class="space-y-4">
                        @if(isset($messages))
                        @include('chat.messages', ['messages' => $messages])
                        @else
                        <div class="mb-7 text-center">
                            <div class="select-none pointer-events-none inline-flex justify-center text-2xl font-semibold leading-9">
                                <h1>How can we help?</h1>
                            </div>
                        </div>
                        @endif
                    </div>
                </div>
            </div>
            <div class="p-4 w-full max-w-[650px] mx-auto">
                @include('dashboard.message-form', ['thread' => $thread ?? null])
            </div>
        </div>
    </div>

    @push('scripts')
    <script>
        document.addEventListener('DOMContentLoaded', function() {
            const messageList = document.getElementById('message-list');

            htmx.on('htmx:afterSwap', function(event) {
                if (event.detail.target.id === 'message-list') {
                    messageList.scrollTop = messageList.scrollHeight;
                }
            });

            // Custom event listener for new messages
            document.addEventListener('newMessage', function(e) {
                const messageHtml = e.detail.html;
                const tempDiv = document.createElement('div');
                tempDiv.innerHTML = messageHtml;
                const newMessage = tempDiv.firstElementChild;
                messageList.appendChild(newMessage);
                messageList.scrollTop = messageList.scrollHeight;
            });
        });
    </script>
    @endpush
</x-layouts.app>
