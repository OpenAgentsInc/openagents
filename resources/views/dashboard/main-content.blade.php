<div id="main-content" class="relative z-10 flex flex-col items-center justify-center min-h-screen p-4">
    <div class="flex h-full flex-col items-center justify-center text-zinc-200">
        <div class="h-full w-full lg:py-[18px]">
            <div class="m-auto text-base px-3 md:px-4 w-full md:px-5 lg:px-4 xl:px-5 h-full">
                <div class="mx-auto flex h-full w-full flex-col text-base justify-center md:max-w-3xl">
                    <div id="messages-container" class="mt-8">
                        <h2 class="text-xl font-semibold mb-4">Messages</h2>
                        <ul class="space-y-4" id="message-list">
                            <!-- Messages will be loaded here -->
                        </ul>
                    </div>
                    @include('dashboard.message-form')
                    @include('dashboard.terms-privacy')
                </div>
            </div>
        </div>
    </div>
</div>

<script>
    document.addEventListener('DOMContentLoaded', function() {
        const messageList = document.getElementById('message-list');

        // Custom event listener for new messages
        document.addEventListener('newMessage', function(e) {
            const messageHtml = e.detail.html;
            const tempDiv = document.createElement('div');
            tempDiv.innerHTML = messageHtml;
            const newMessage = tempDiv.firstElementChild;
            messageList.insertBefore(newMessage, messageList.firstChild);
            console.log('New message added:', messageHtml); // Add this line for debugging
        });
    });
</script>