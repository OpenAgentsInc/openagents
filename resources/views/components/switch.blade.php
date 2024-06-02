<div x-data="{ enabled: @entangle($attributes->wire('model')) }" class="flex items-center justify-between w-full">
    <span class="flex flex-grow flex-col p-3">
        <span class="text-sm font-medium leading-6 text-gray" id="availability-label">@if(isset($label)){{ $label }}@endif</span>
        <span class="text-sm text-slate-100" id="availability-description">@if(isset($description)){{ $description }}@endif</span>
    </span>
    <!-- Enabled: "bg-indigo-600", Not Enabled: "bg-gray-200" -->
    <button @click="enabled = !enabled" x-bind:class="{ 'bg-white': enabled, 'bg-black border border-white': !enabled }" type="button" class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-indigo-600 focus:ring-offset-2" role="switch" aria-checked="false" aria-labelledby="availability-label" aria-describedby="availability-description">
        <!-- Enabled: "translate-x-5", Not Enabled: "translate-x-0" -->
        <span x-bind:class="{ 'translate-x-5 bg-black': enabled, 'translate-x-0 bg-white': !enabled }" aria-hidden="true" class="pointer-events-none inline-block h-5 w-5 transform rounded-full   shadow ring-0 transition duration-200 ease-in-out"></span>
    </button>
    @if ($attributes->has('wire:model'))
        <input {{ $attributes->except('model') }} type="hidden" wire:model="{{ $attributes->wire('model', 'enabled') }}" x-bind:value="enabled ? 'true' : 'false'">
        @else
        <input {{ $attributes->except('model') }} type="hidden" x-bind:value="enabled ? 'true' : 'false'">
    @endif
</div>
