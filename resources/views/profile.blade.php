<x-htmx-layout>
    <main class="p-12">
        <div class="flex flex-row gap-x-6">
            <img src="{{ str_replace('_normal', '', auth()->user()->profile_photo_path) }}"
                 alt="{{ auth()->user()->name }}"
                 class="rounded-xl w-[120px] h-[120px]"/>
            <div>
                <h1>{{ auth()->user()->name }}</h1>
                <h2><span class="mr-[4px] text-text">@</span>{{ auth()->user()->username }}</h2>
            </div>
        </div>
        <div class="mx-2 my-4">
            <a href="https://x.com/{{ auth()->user()->username }}" target="_blank">
                <x-icon.x class="h-6 w-6"/>
            </a>
        </div>
    </main>
</x-htmx-layout>