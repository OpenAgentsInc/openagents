<?php

namespace App\View\Components;

use Illuminate\View\Component;
use Illuminate\Support\Str;

class Dropdown extends Component
{
    public string $uuid;

    public function __construct(
        public string $label,
        public array $items,
        public ?string $id = null,
        public bool $checkable = false,
        public ?string $icon = null,
        public ?string $class = '',
        public bool $loading = false
    ) {
        $this->uuid = $id ?? Str::uuid();
    }

    public function render()
    {
        return <<<'BLADE'
            <div x-data="{ open: false }" 
                 class="relative inline-block text-left {{ $class }}"
                 id="{{ $uuid }}">
                <div>
                    <button @click="open = !open" type="button" 
                            class="inline-flex justify-between w-full rounded-md border border-sidebar-border bg-transparent px-4 py-2 text-sm font-medium text-sidebar-foreground shadow-sm hover:bg-sidebar-accent focus:outline-none focus:ring-0 focus:ring-sidebar-ring" 
                            aria-haspopup="true" x-bind:aria-expanded="open.toString()"
                            x-bind:disabled="@js($loading)">
                        <span class="flex items-center">
                            @if($icon)
                                <span class="mr-2">{!! $icon !!}</span>
                            @endif
                            {{ $label }}
                        </span>
                        @if($loading)
                            <svg class="animate-spin h-5 w-5 text-sidebar-foreground" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                        @else
                            <svg class="h-5 w-5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                                <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                            </svg>
                        @endif
                    </button>
                </div>

                <div x-show="open" @click.away="open = false" class="origin-top-right absolute right-0 mt-2 w-full rounded-md shadow-lg bg-sidebar-background ring-1 ring-sidebar-border ring-opacity-5 divide-y divide-sidebar-border z-50">
                    <div class="py-1" role="menu" aria-orientation="vertical" aria-labelledby="{{ $uuid }}">
                        @if($loading)
                            <div class="px-4 py-2 text-sm text-sidebar-foreground">Loading...</div>
                        @else
                            @foreach($items as $item)
                                @if($checkable)
                                    <div class="flex items-center px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground cursor-pointer" role="menuitem">
                                        <input type="checkbox" class="form-checkbox h-4 w-4 text-sidebar-accent-foreground rounded border-sidebar-border mr-2" id="{{ $uuid }}-{{ $loop->index }}">
                                        <label for="{{ $uuid }}-{{ $loop->index }}" class="flex-grow cursor-pointer">{{ $item }}</label>
                                    </div>
                                @else
                                    <a href="#" class="block px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground" role="menuitem">{{ $item }}</a>
                                @endif
                            @endforeach
                        @endif
                    </div>
                </div>
            </div>
        BLADE;
    }
}