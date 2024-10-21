<form id="chat-form" class="mt-4" hx-post="{{ isset($thread) ? route('chat.send', $thread) : route('threads.create') }}" hx-target="#message-list" hx-swap="beforeend">
    @csrf
    <div class="flex items-center">
        <input type="text" name="content" class="flex-grow mr-2 p-2 rounded-md bg-gray-700 text-white" placeholder="Type your message..." required>
        <button type="submit" class="bg-blue-500 text-white px-4 py-2 rounded-md">Send</button>
    </div>
</form>

<script>
    document.getElementById('chat-form').addEventListener('htmx:afterRequest', function(event) {
        if (event.detail.successful) {
            event.target.reset();
        }
    });
</script>