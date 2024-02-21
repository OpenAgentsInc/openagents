<x-blank-layout>
    <livewire:navbar />

    <div class="max-w-8xl mx-auto px-4 sm:px-6 md:px-8">
        <div
            class="z-20 hidden lg:block fixed bottom-0 right-auto w-[18rem] pl-4 pr-6 pb-10 overflow-y-auto stable-scrollbar-gutter top-[7.3rem]">
            <div class="relative lg:text-sm lg:leading-6">
                <ul>
                    @foreach($documentsList as $slug => $title)
                        <li
                            class="my-2 {{ $activePage === $slug ? 'text-white' : 'text-gray' }}">
                            <a wire:navigate
                                href="{{ route('docs.show', $slug) }}">{{ $title }}</a>
                        </li>
                    @endforeach
                </ul>
            </div>
        </div>

        <div class="flex justify-center min-h-screen">
            <div class="docs w-full max-w-5xl pt-20 px-12">
                {!! $content->contents !!}
            </div>
        </div>
    </div>
</x-blank-layout>
