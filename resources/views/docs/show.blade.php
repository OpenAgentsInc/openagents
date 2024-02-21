<x-blank-layout>
    <livewire:navbar />

    <aside class="fixed px-6 pt-28 w-[300px]">
    <ul>
        @foreach ($documentsList as $slug => $title)
            <li class="my-2 {{ $activePage === $slug ? 'text-white' : 'text-gray' }}">
                <a wire:navigate href="{{ route('docs.show', $slug) }}">{{ $title }}</a>
            </li>
        @endforeach
    </ul>
    </aside>

    <div class="flex justify-center min-h-screen">
        <div class="docs w-full max-w-5xl pt-20 px-12">
            {!! $content->contents !!}
        </div>
    </div>
</x-blank-layout>
