@props(['id' => null, 'maxWidth' => null])

<x-modal :id="$id" :maxWidth="$maxWidth" {{ $attributes }}>
    <div class="px-6 py-4">
        <div class="text-lg font-medium text-white">
            {{ $title }}
        </div>

        <div class="mt-4 text-sm text-gray ">
            {{ $content }}
        </div>
    </div>

    <div class="flex flex-row justify-end px-6 py-4 bg-black text-end">
        {{ $footer }}
    </div>
</x-modal>
