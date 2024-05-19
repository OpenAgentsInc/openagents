<x-htmx-layout>
    <main class="p-12 relative"> <!-- Add 'relative' class to the main element -->
        <!-- X icon positioned top right -->


        <div class="flex flex-row gap-x-6">
            @if($user->profile_photo_path)
                <img src="{{ str_replace('_normal', '', $user->profile_photo_path) }}"
                     alt="{{ $user->name }}"
                     class="rounded-xl w-[120px] h-[120px]"/>
            @else
                <img src="/images/nostrich.jpeg"
                     alt="{{ $user->name }}"
                     class="rounded-xl w-[120px] h-[120px]"/>
            @endif
            <div class="flex flex-col justify-center">
                <h1>{{ $user->name }}</h1>
                @if ($user->username)
                    <h2><span class="mr-[4px] text-text">@</span>{{ $user->username }}</h2>
                @endif
            </div>
            @if ($user->username)
                <div class="flex flex-col justify-center">
                    <a href="https://x.com/{{ $user->username }}" target="_blank"
                       class="p-1.5 border border-offblack hover:bg-offblack rounded">
                        <x-icon.x class="h-6 w-6"/>
                    </a>
                </div>
            @endif
        </div>

        <h3 class="mt-12 mb-6">Agents</h3>
        <div class="grid grid-cols-2 sm:grid-cols-3 gap-6">
            @foreach($user->agents as $agent)
                <livewire:agent-card :agent="$agent" :key="$agent->id"/>
            @endforeach
        </div>
    </main>
</x-htmx-layout>
