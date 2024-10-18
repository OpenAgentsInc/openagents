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

    public function modelName(): ?string
    {
        return $this->attributes->whereStartsWith('wire:model')->first();
    }

    public function errorFieldName(): ?string
    {
        return $this->errorField ?? $this->modelName();
    }

    public function moneySettings(): string
    {
        return json_encode([
            'init' => true,
            'maskOpts' => [
                'locales' => $this->locale
            ]
        ]);
    }

    public function render(): View|Closure|string
    {
        return <<<'BLADE'
            <div>
                @php
                    // We need this extra step to support models arrays. Ex: wire:model="emails.0"  , wire:model="emails.1"
                    $uuid = $uuid . $modelName()
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
                                "!border-destructive" => $errorFieldName() && $errors->has($errorFieldName()) && !$omitError
                            ])
                    >
                        {{ $prepend ?? $prefix }}
                    </div>
                @endif

                <div class="flex-1 relative">
                    {{-- MONEY SETUP --}}
                    @if($money)
                        <div
                            wire:key="money-{{ rand() }}"
                            x-data="{ amount: $wire.get('{{ $modelName() }}') }" x-init="$nextTick(() => new Currency($refs.myInput, {{ $moneySettings() }}))"
                        >
                    @endif

                    {{-- INPUT --}}
                    <input
                        id="{{ $uuid }}"
                        placeholder = "{{ $attributes->whereStartsWith('placeholder')->first() }} "

                        @if($money)
                            x-ref="myInput"
                            :value="amount"
                            x-on:input="$nextTick(() => $wire.set('{{ $modelName() }}', Currency.getUnmasked(), false))"
                            inputmode="numeric"
                        @endif

                        {{
                            $attributes
                                ->merge(['type' => 'text'])
                                ->except($money ? 'wire:model' : '')
                                ->class([
                                    'peer flex w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
                                    'ps-10' => ($icon),
                                    'h-14' => ($inline),
                                    'pt-3' => ($inline && $label),
                                    'rounded-s-none' => $prefix || $prepend,
                                    'rounded-e-none' => $suffix || $append,
                                    'border border-dashed' => $attributes->has('readonly') && $attributes->get('readonly') == true,
                                    'input-error' => $errorFieldName() && $errors->has($errorFieldName()) && !$omitError,
                            ])
                        }}
                        style="--tw-ring-width: 0;"
                    />

                    {{-- ICON  --}}
                    @if($icon)
                        <x-mary-icon :name="$icon" class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none" />
                    @endif

                    {{-- CLEAR ICON  --}}
                    @if($clearable)
                        <x-mary-icon x-on:click="$wire.set('{{ $modelName() }}', '', {{ json_encode($attributes->wire('model')->hasModifier('live')) }})"  name="o-x-mark" class="absolute right-3 top-1/2 -translate-y-1/2 cursor-pointer text-muted-foreground hover:text-foreground" />
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

                    {{-- HIDDEN MONEY INPUT + END MONEY SETUP --}}
                    @if($money)
                            <input type="hidden" {{ $attributes->only('wire:model') }} />
                        </div>
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
                                "!border-error" => $errorFieldName() && $errors->has($errorFieldName()) && !$omitError
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
                @if(!$omitError && $errors->has($errorFieldName()))
                    @foreach($errors->get($errorFieldName()) as $message)
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