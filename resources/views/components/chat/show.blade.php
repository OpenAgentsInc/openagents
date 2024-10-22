<div class="mx-auto flex h-full w-full flex-col text-base justify-between md:max-w-3xl">
    <div class="flex-grow overflow-y-auto">
        <h2 class="text-xl font-bold mb-4">{{ $thread->title }}</h2>
        <x-chat.messages :messages="$messages" />
    </div>
    <div class="mt-4">
        <form id="message-form" hx-post="{{ route('chat.send', $thread) }}" hx-target="#message-list" hx-swap="beforeend">
            @csrf
            <div class="flex items-center">
                <input type="text" name="content" class="flex-grow mr-2 p-2 border rounded" placeholder="Type your message...">
                <button type="submit" class="bg-blue-500 text-white px-4 py-2 rounded">Send</button>
            </div>
        </form>
        @include('dashboard.terms-privacy')
    </div>
</div>

<script>
    document.addEventListener('DOMContentLoaded', function() {
        const messageForm = document.getElementById('message-form');
        if (messageForm) {
            messageForm.addEventListener('htmx:afterRequest', function(event) {
                if (event.detail.successful) {
                    event.detail.elt.reset();
                }
            });
        }
    });
</script>