<?php

namespace App\View\Components;

use Illuminate\View\Component;
use Illuminate\Support\Str;

class Input extends Component
{
    public string $uuid;

    public function __construct(
        public ?string $label = null,
        public ?string $type = 'text',
        public ?string $name = null,
        public ?string $id = null,
        public ?string $placeholder = null,
        public ?string $value = null,
        public bool $required = false,
        public ?string $icon = null,
    ) {
        $this->uuid = $id ?? Str::uuid();
    }

    public function render()
    {
        return <<<'blade'
            <div>
                <div class="relative">
                    @if($icon)
                        <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                            <svg class="h-5 w-5 text-muted-foreground" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                                <path fill-rule="evenodd" d="{{ self::getIconPath($icon) }}" clip-rule="evenodd" />
                            </svg>
                        </div>
                    @endif
                    <input
                        type="{{ $type }}"
                        name="{{ $name }}"
                        id="{{ $uuid }}"
                        placeholder="{{ $placeholder }}"
                        value="{{ $value }}"
                        @if($required) required @endif
                        {{ $attributes->merge(['class' => 'peer flex w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50' . ($icon ? ' pl-10' : '')]) }}
                    >
                    @if($label)
                        <label
                            for="{{ $uuid }}"
                            class="absolute text-sm text-muted-foreground duration-300 transform -translate-y-4 scale-75 top-2 z-10 origin-[0] bg-background px-2 peer-focus:px-2 peer-focus:text-primary peer-placeholder-shown:scale-100 peer-placeholder-shown:-translate-y-1/2 peer-placeholder-shown:top-1/2 peer-focus:top-2 peer-focus:scale-75 peer-focus:-translate-y-4 left-1"
                        >
                            {{ $label }}
                        </label>
                    @endif
                </div>
            </div>
        blade;
    }

    private static function getIconPath($icon)
    {
        $paths = [
            'user' => 'M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z',
            'envelope' => 'M2.003 5.884L10 9.882l7.997-3.998A2 2 0 0016 4H4a2 2 0 00-1.997 1.884z',
            'lock-closed' => 'M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z',
        ];

        return $paths[$icon] ?? '';
    }
}