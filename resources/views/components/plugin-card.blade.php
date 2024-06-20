@props(['plugin'])



<div class="select-auto pointer-events-auto border border-offblack hover:border-darkgray rounded-lg p-4 flex flex-col leading-normal"
>
    <div class="mt-1 mb-3 w-[20px] h-[20px] sm:w-[60px] sm:h-[60px]">
        <img src="{{$plugin->getImageUrlAttribute()}}" alt="Plugin" class="w-full h-full rounded" />
    </div>

    <div class="font-bold text-xl">{{ $plugin->name }}</div>
    <div class="flex items-center">
        <div class="text-xs">
            <p class="text-sm text-gray leading-none">By: {{ $plugin->user->name }}</p>
        </div>
        <img class="w-5 h-5 rounded-full mx-4" src="{{ $plugin->user->profile_photo_path }}"
             alt="Avatar of {{ $plugin->user->name }}" />
    </div>
    <div class="flex-grow">
        <p class="text-sm text-text my-1">{{ $plugin->description }}</p>
    </div>

    @if ($plugin->suspended && auth()->check() && $plugin->isEditableBy(auth()->user()))
        <div class="mt-5 w-full">
            <p class="my-2 text-red text-sm text-center font-bold">
                {{ $plugin->suspended }}
            </p>
        </div>
    @elseif ($plugin->pending_revision && auth()->check() && $plugin->isEditableBy(auth()->user()))
        <div class="mt-5 w-full">
            <p class="my-2 text-purple-500 text-sm text-center font-bold">
                {{ $plugin->pending_revision_reason }}
            </p>
        </div>
    @endif

    <p class="mb-0 text-sm text-gray leading-none">Price: {{ $plugin->price_msats/1000 }} sats per call</p>


   <div class="text-gray mt-4 gap-x-6 flex justify-end items-center">
        <div class="flex items-center">
            <x-icon.bitcoin class="w-4 h-4 mr-1" />
            <span>{{ $plugin->sats_earned ?? 0 }}</span>
        </div>
    </div>

    <div class="text-gray mt-4 gap-x-6 flex justify-end items-center">
        @if ($plugin->web)
            <a href="{{ $plugin->web }}" target="_blank" title="Visit website">
                <x-icon.link class="h-4 w-4 text-white" style="stroke: white;" fill="grey" />
            </a>
        @endif
        @if ($plugin->web)
            <a href="{{ $plugin->tos }}" target="_blank" title="Terms of Service">
                <x-icon.law class="h-4 w-4 text-white" style="stroke: white;" fill="grey" />
            </a>
        @endif
        @if ($plugin->privacy)
            <a href="{{ $plugin->privacy }}" target="_blank" title="Privacy Policy">
                <x-icon.privacy class="h-4 w-4 text-white" style="stroke: white;" fill="grey" />
            </a>
        @endif
        @if (auth()->check() && $plugin->isEditableBy(auth()->user()))
            <a class="flex items-center"
            href="{{ route('plugins.edit', ['plugin' => $plugin]) }}" wire:navigate title="Edit">
                <x-icon.pen role='button' class="w-4 h-4 mr-1" />
            </a>
            <a class="flex items-center" x-on:click="Livewire.dispatch('openModal', { component: 'plugins.modals.delete', arguments: { plugin: {{ $plugin->id }} } })" title="Delete">
                <x-icon.trash role='button' class="w-4 h-4 mr-1" />
            </a>
        @endif
    </div>
</div>

