<x-layouts.app>
    <div class="relative h-screen overflow-hidden bg-background">
        <div class="relative z-10 flex flex-col h-screen">
            <div id="main-content" class="flex-grow overflow-y-auto flex items-center justify-center">
                <div class="w-full max-w-3xl px-4">
                    @include('components.chat.messages')
                </div>
            </div>
            <div class="p-4 w-full max-w-[650px] mx-auto">
                <x-dashboard.message-form :thread="$thread ?? null" />
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