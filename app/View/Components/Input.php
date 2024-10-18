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
        public ?string $value = null,
        public bool $required = false,
        public ?string $icon = null,
        public bool $inline = false
    ) {
        $this->uuid = $id ?? Str::uuid();
    }

    public function render()
    {
        return <<<'BLADE'
            <div>
                <div class="relative">
                    @if($icon)
                        <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                            {!! $icon !!}
                        </div>
                    @endif
                    <input
                        type="{{ $type }}"
                        name="{{ $name }}"
                        id="{{ $uuid }}"
                        value="{{ $value }}"
                        placeholder=" "
                        @if($required) required @endif
                        {{ $attributes->merge(['class' => 'peer flex w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 pt-3 h-14' . ($icon ? ' pl-10' : '')]) }}
                    >
                    @if($label)
                        <label
                            for="{{ $uuid }}"
                            class="absolute text-muted-foreground duration-300 transform -translate-y-4 scale-75 top-2 z-10 origin-[0] bg-transparent px-2 peer-focus:px-2 peer-focus:text-zinc-600 peer-focus:dark:text-zinc-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:-translate-y-1/2 peer-placeholder-shown:top-1/2 peer-focus:top-2 peer-focus:scale-75 peer-focus:-translate-y-4 {{ $icon ? 'left-9' : 'left-1' }} rtl:peer-focus:translate-x-1/4 rtl:peer-focus:left-auto rtl:peer-focus:right-8"
                        >
                            {{ $label }}
                        </label>
                    @endif
                </div>
            </div>
        BLADE;
    }
}