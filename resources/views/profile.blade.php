<x-htmx-layout>
    <main class="p-12">
        <div class="flex flex-row gap-x-6">
            <img src="{{ str_replace('_normal', '', $user->profile_photo_path) }}"
                 alt="{{ $user->name }}"
                 class="rounded-xl w-[120px] h-[120px]"/>
            <div>
                <h1>{{ $user->name }}</h1>
                <h2><span class="mr-[4px] text-text">@</span>{{ $user->username }}</h2>
            </div>
        </div>
        <div class="mx-2 my-4">
            <a href="https://x.com/{{ $user->username }}" target="_blank">
                <x-icon.x class="h-6 w-6"/>
            </a>
        </div>
    </main>
</x-htmx-layout>