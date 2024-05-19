<x-htmx-layout>
    <div class="flex flex-col w-full relative z-50 h-full">
        <div class="flex-1 overflow-y-auto">
            <div class="flex flex-col gap-2 py-3 px-1">
                <x-htmx.threads-list :threads="[]"/>
            </div>
        </div>
    </div>
</x-htmx-layout>
