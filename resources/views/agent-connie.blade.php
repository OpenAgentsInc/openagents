<!-- A ChatGPT-style chatbot interface in Tailwind, all black/white darkmode -->
<x-app-layout>
    <div class="flex flex-col h-screen">
        <!-- Chat Container -->
        <div class="max-w-7xl mx-auto flex flex-col h-full">
            <div class="flex-1 bg-black overflow-hidden">
                <!-- Messages Display Area -->
                <div id="messagesDisplay" class="flex-1 overflow-y-auto p-4 space-y-4">
                    <!-- Example of a message bubble -->
                    <div class="flex items-end justify-end">
                        <div class="max-w-xs lg:max-w-md bg-offblack text-white p-3 rounded-lg">
                            This is a message from the user.
                        </div>
                    </div>
                    <div class="flex items-start justify-start">
                        <div class="max-w-xs lg:max-w-md bg-darkgray text-white p-3 rounded-lg">
                            This is a reply from the chatbot.
                        </div>
                    </div>
                    <!-- Additional messages will be inserted here -->
                </div>
            </div>

            <!-- Message Input Area -->
            <div class="px-4 py-2 bg-black">
                <form id="messageForm" action="#" method="POST" class="flex space-x-2">
                    @csrf
                    <x-input type="text" id="messageInput" placeholder="Type a message..." class="flex-1 bg-darkgray text-white p-2 rounded-lg focus:outline-none" autocomplete="off" />
                    <button type="submit" class="bg-gray text-white px-4 py-2 rounded-lg hover:bg-lightgray focus:outline-none">
                        Send
                    </button>
                </form>
            </div>
        </div>
    </div>
</x-app-layout>
