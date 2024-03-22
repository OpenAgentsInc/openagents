<div>
    {{-- Nothing in the world is as soft and yielding as water. --}}

    <div class="max-w-2xl mx-auto my-11">


        <form wire:submit.prevent='sendMessage'>
            <label for="chat" class="sr-only">Your message</label>
            <div class="flex items-center py-2 px-3 bg-gray-50 rounded-lg dark:bg-gray-700">

                <textarea id="chat" wire:model='message' rows="1" class="block mx-4 p-2.5 w-full text-sm text-gray-900 bg-black rounded-lg border border-gray-300 focus:ring-gray-500 focus:border-gray-500 dark:bg-gray-800 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-gray-500 dark:focus:border-gray-500" placeholder="Your message..."></textarea>
                <button type="submit" class="inline-flex justify-center p-2 text-gray-600 rounded-full cursor-pointer hover:bg-gray-100 dark:text-gray-500 dark:hover:bg-gray-600">
                    <svg class="w-6 h-6 rotate-90" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                        <path d="M10.894 2.553a1 1 0 00-1.788 0l-7 14a1 1 0 001.169 1.409l5-1.429A1 1 0 009 15.571V11a1 1 0 112 0v4.571a1 1 0 00.725.962l5 1.428a1 1 0 001.17-1.408l-7-14z"></path>
                    </svg>
                </button>
            </div>
        </form>


        <div class="w-full px-5 flex flex-col justify-between my-4 divide-y divide-gray-400">

            @forelse ($this->chats as $chat)
            <div class="flex flex-col">

                <div class="flex justify-start my-4">
                    <img src="https://source.unsplash.com/vpOeXr5wmR4/600x600" class="object-cover h-8 w-8 rounded-full" alt="">
                    <div class="ml-2 py-3 px-4 bg-gray-400 rounded-br-3xl rounded-tr-3xl rounded-tl-xl text-white">
                       {{$chat['message']}}
                    </div>
                </div>


            </div>
            @empty

            @endforelse

        </div>

    </div>
</div>
