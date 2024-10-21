<form hx-post="{{ route('chat.send', $thread ?? 0) }}" hx-trigger="submit" class="flex items-center">
    @csrf
    <input type="text" name="content" placeholder="Type your message..." class="flex-grow p-2 border rounded-l-lg focus:outline-none focus:ring-2 focus:ring-blue-500">
    <button type="submit" class="bg-blue-500 text-white p-2 rounded-r-lg hover:bg-blue-600 focus:outline-none focus:ring-2 focus:ring-blue-500">Send</button>
</form>
<script>
    document.body.addEventListener('htmx:afterRequest', function(event) {
        if (event.detail.elt.getAttribute('hx-post') === "{{ route('chat.send', $thread ?? 0) }}") {
            event.detail.elt.reset();
        }
    });
</script>