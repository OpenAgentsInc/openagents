<?php

namespace App\View\Components;

use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\View\Component;

class Input extends Component
{
    public string $uuid;

    public function __construct(
        public ?string $label = null,
        public ?string $icon = null,
        public ?string $iconRight = null,
        public ?string $hint = null,
        public ?string $hintClass = 'text-muted-foreground text-sm py-1 pb-0',
        public ?string $prefix = null,
        public ?string $suffix = null,
        public ?bool $inline = false,
        public ?bool $clearable = false,
        public ?bool $money = false,
        public ?string $locale = 'en-US',

        // Slots
        public mixed $prepend = null,
        public mixed $append = null,
        // Validations
        public ?string $errorField = null,
        public ?string $errorClass = 'text-destructive text-sm p-1',
        public ?bool $omitError = false,
        public ?bool $firstErrorOnly = false,
    ) {
        $this->uuid = "mary" . md5(serialize($this));
    }

    public function render(): View|Closure|string
    {
        return <<<'BLADE'
            <div>
                @php
                    $modelName = $attributes->whereStartsWith('name')->first();
                    $uuid = $uuid . $modelName;
                @endphp

                {{-- STANDARD LABEL --}}
                @if($label && !$inline)
                    <label for="{{ $uuid }}" class="pt-0 label label-text font-semibold">
                        <span>
                            {{ $label }}

                            @if($attributes->get('required'))
                                <span class="text-destructive">*</span>
                            @endif
                        </span>
                    </label>
                @endif

                {{-- PREFIX/SUFFIX/PREPEND/APPEND CONTAINER --}}
                @if($prefix || $suffix || $prepend || $append)
                    <div class="flex">
                @endif

                {{-- PREFIX / PREPEND --}}
                @if($prefix || $prepend)
                    <div
                        @class([
                                "flex items-center rounded-l-md border border-r-0 border-border bg-muted px-3 text-sm text-muted-foreground",
                                "border border-primary border-e-0 px-4" => $prefix,
                                "border-0 bg-muted-foreground" => $attributes->has('disabled') && $attributes->get('disabled') == true,
                                "border-dashed" => $attributes->has('readonly') && $attributes->get('readonly') == true,
                                "!border-destructive" => $errorField && $errors->has($errorField) && !$omitError
                            ])
                    >
                        {{ $prepend ?? $prefix }}
                    </div>
                @endif

                <div class="flex-1 relative">
                    {{-- INPUT --}}
                    <input
                        id="{{ $uuid }}"
                        placeholder="{{ $attributes->whereStartsWith('placeholder')->first() }} "
                        {{
                            $attributes
                                ->merge(['type' => 'text'])
                                ->class([
                                    'peer flex w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
                                    'ps-10' => ($icon),
                                    'h-14' => ($inline),
                                    'pt-3' => ($inline && $label),
                                    'rounded-s-none' => $prefix || $prepend,
                                    'rounded-e-none' => $suffix || $append,
                                    'border border-dashed' => $attributes->has('readonly') && $attributes->get('readonly') == true,
                                    'input-error' => $errorField && $errors->has($errorField) && !$omitError,
                            ])
                        }}
                        style="--tw-ring-width: 0;"
                    />

                    {{-- ICON  --}}
                    @if($icon)
                        <x-mary-icon :name="$icon" class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none" />
                    @endif

                    {{-- RIGHT ICON  --}}
                    @if($iconRight)
                    <x-mary-icon :name="$iconRight" @class(["absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none", "!right-10" => $clearable]) />
                    @endif

                    {{-- INLINE LABEL --}}
                    @if($label && $inline)
                        <label for="{{ $uuid }}" class="absolute text-muted-foreground duration-300 transform -translate-y-1 scale-75 top-2 origin-left rtl:origin-right rounded px-2 peer-focus:px-2 peer-focus:text-zinc-600 peer-focus:dark:text-zinc-500 peer-placeholder-shown:scale-100 peer-placeholder-shown:-translate-y-1/2 peer-placeholder-shown:top-1/2 peer-focus:top-2 peer-focus:scale-75 peer-focus:-translate-y-1 @if($inline && $icon) start-9 @else start-3 @endif">
                            {{ $label }}
                        </label>
                    @endif
                </div>

                {{-- SUFFIX/APPEND --}}
                @if($suffix || $append)
                     <div
                        @class([
                                "rounded-e-xl flex items-center bg-muted px-3 text-sm text-muted-foreground",
                                "border border-primary border-s-0 px-4" => $suffix,
                                "border-0 bg-base-300" => $attributes->has('disabled') && $attributes->get('disabled') == true,
                                "border-dashed" => $attributes->has('readonly') && $attributes->get('readonly') == true,
                                "!border-error" => $errorField && $errors->has($errorField) && !$omitError
                            ])
                    >
                        {{ $append ?? $suffix }}
                    </div>
                @endif

                {{-- END: PREFIX/SUFFIX/APPEND/PREPEND CONTAINER  --}}
                @if($prefix || $suffix || $prepend || $append)
                    </div>
                @endif

                {{-- ERROR --}}
                @if(!$omitError && $errorField && $errors->has($errorField))
                    @foreach($errors->get($errorField) as $message)
                        @foreach(Arr::wrap($message) as $line)
                            <div class="{{ $errorClass }}" x-classes="text-destructive text-sm p-1">{{ $line }}</div>
                            @break($firstErrorOnly)
                        @endforeach
                        @break($firstErrorOnly)
                    @endforeach
                @endif

                {{-- HINT --}}
                @if($hint)
                    <div class="{{ $hintClass }}" x-classes="text-muted-foreground text-sm py-1 pb-0">{{ $hint }}</div>
                @endif
            </div>
            BLADE;
    }
}