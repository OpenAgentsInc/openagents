<div  class="flex items-center justify-between w-full">
    <span class="flex flex-grow flex-col p-3">
        <span class="text-sm font-medium leading-6 text-gray" id="availability-label">{{ $label }}</span>
        <span class="text-sm text-slate-100" id="availability-description">{{ $description }}</span>
    </span>

    <div class="flex h-6 items-center">
        <input {{ $attributes->except('class') }} type="checkbox" class="h-4 w-4 rounded border border-gray/90 text-black ring-gray focus:ring-white">
      </div>



</div>
