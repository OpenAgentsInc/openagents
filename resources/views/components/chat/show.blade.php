<div class="mx-auto flex h-full w-full flex-col text-base justify-between md:max-w-3xl">
    <div class="flex-grow overflow-y-auto">
        <h2 class="text-xl font-bold mb-4">{{ $thread->title }}</h2>
        <x-chat.messages :messages="$messages" />
    </div>
    <div class="mt-4">
        @include('dashboard.message-form', ['thread' => $thread])
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