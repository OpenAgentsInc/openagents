<div class="w-full px-3">
    <div class="sm:w-[584px] lg:w-[768px] mx-auto relative">
        <form hx-post="/message" hx-trigger="submit" hx-swap="none" hx-on="htmx:afterRequest: resetTextarea()">
            @csrf
            <textarea id="message-input" name="message-input" minRows="1" placeholder="Message OpenAgents..."
                      min-rows="1" max-rows="12"
                      autofocus
                      dusk="message-input"
                      onkeydown="if(event.keyCode === 13 && !event.shiftKey && window.innerWidth > 768) { event.preventDefault(); document.getElementById('send-message').click(); }"
                      class="flex h-[48px] w-full rounded-md border-2 bg-transparent p-3 pr-10 text-[16px] placeholder:text-[#777A81]
                       focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white
                       resize-none"></textarea>
            <button type="submit" id="send-message"
                    class="absolute bottom-[10px] right-[10px] text-black shadow rounded bg-white hover:bg-white/90"
                    dusk="send-message-button"
            >
                <x-icon name="send" class="w-[24px] h-[24px] m-0.5 flex flex-col justify-center items-center"/>
            </button>
        </form>
    </div>
</div>

<script>
    function resetTextarea() {
        document.getElementById('message-input').value = '';
    }
</script>