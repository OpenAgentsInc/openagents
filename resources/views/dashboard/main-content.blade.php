<main class="relative z-10 flex flex-col items-center justify-center min-h-screen p-4">
    <div class="flex h-full flex-col items-center justify-center text-zinc-200">
        <div class="h-full w-full lg:py-[18px]">
            <div class="m-auto text-base px-3 md:px-4 w-full md:px-5 lg:px-4 xl:px-5 h-full">
                <div class="mx-auto flex h-full w-full flex-col text-base justify-center md:max-w-3xl">
                    <div class="mb-7 text-center block">
                        <div class="select-none pointer-events-none inline-flex justify-center text-2xl font-semibold leading-9">
                            <h1>How can we help?</h1>
                        </div>
                    </div>
                    @include('dashboard.message-form')
                    
                    @if($messages->isNotEmpty())
                        <div class="mt-8">
                            <h2 class="text-xl font-semibold mb-4">Your Messages</h2>
                            <ul class="space-y-4">
                                @foreach($messages as $message)
                                    <li class="bg-zinc-800 p-4 rounded-lg">
                                        <p class="text-sm text-zinc-400">{{ $message->created_at->format('M d, Y H:i') }}</p>
                                        <p class="mt-2">{{ $message->content }}</p>
                                    </li>
                                @endforeach
                            </ul>
                        </div>
                    @endif

                    @include('dashboard.terms-privacy')
                </div>
            </div>
        </div>
    </div>
</main>