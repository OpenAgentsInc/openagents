<div id="chatbox-container" x-ref="chatboxContainer" class="overflow-y-scroll h-full">
    <div class="relative"
         x-data="{ pending: @entangle('pending').live, autoscroll: {{ $autoscroll ? 'true' : 'false' }} }"
         x-init="
                let chatbox = $refs.chatboxContainer;
                setTimeout(() => {
                    if (autoscroll) {
                        chatbox.scrollTo({ top: chatbox.scrollHeight, behavior: 'smooth' });
                    }
                }, 250);
                let lastScrollHeight = chatbox.scrollHeight;

                $watch('pending', value => {
                    if (value && autoscroll) {
                        let scrollInterval = setInterval(() => {
                            if (chatbox.scrollHeight != lastScrollHeight) {
                                chatbox.scrollTo({ top: chatbox.scrollHeight, behavior: 'smooth' });
                                lastScrollHeight = chatbox.scrollHeight;
                            }

                            if (!pending) clearInterval(scrollInterval);
                        }, 1);
                    }
                });

                $nextTick(() => {
                    document.getElementById('message-input').focus(); // Focus the textarea
                    $wire.on('no-more-messages', () => {
                        setTimeout(() => {
                            if (autoscroll) {
                                chatbox.scrollTo({ top: chatbox.scrollHeight, behavior: 'smooth' });
                            }
                        }, 250);
                    });
                });
            ">
        {{ $slot }}
    </div>

</div>