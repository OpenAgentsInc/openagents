<div class="w-full px-3">
    <div class="mx-auto mt-12">
        <div class="flex flex-row justify-between">
            <h2 class="mb-4 font-bold">Codebases ({{ count($codebases) }})</h2>
        </div>

        <div class="flex flex-row w-[500px]">
            <form wire:submit.prevent="indexRepo()" class="flex flex-row w-full">
                <x-input type="text" class="w-full" placeholder="Add GitHub link to index" wire:model="index_link"/>
                <x-button type="submit" class="ml-4">Index</x-button>
            </form>
            <x-button wire:click="checkRepo()" class="ml-4">Check</x-button>
        </div>

        <div class="flex flex-col gap-8 mt-6">
            <table class="min-w-full table-fixed divide-y divide-offblack text-white">
                <thead>
                <tr>
                    <th class="p-2 text-left text-sm font-semibold text-gray">
                        <div>Repository</div>
                    </th>
                    <th class="p-2 text-left text-sm font-semibold text-gray">
                        <div>Branch</div>
                    </th>
                    <th class="p-2 text-left text-sm font-semibold text-gray">
                        <div>Remote</div>
                    </th>
                    <th class="p-2 text-left text-sm font-semibold text-gray">
                        <div>Status</div>
                    </th>
                    <th class="p-2 text-left text-sm font-semibold text-gray">
                        <div>Files Processed</div>
                    </th>
                    <th class="p-2 text-left text-sm font-semibold text-gray">
                        <div>SHA</div>
                    </th>
                    <th class="p-2 text-left text-sm font-semibold text-gray">
                        <div>Created At</div>
                    </th>
                </tr>
                </thead>
                <tbody class="divide-y divide-offblack bg-black text-gray">
                @foreach($codebases as $codebase)
                    <tr wire:key="{{ $codebase->id }}"
                        class="cursor-pointer hover:bg-offblack hover:bg-opacity-50 transition-colors duration-50 ease-in-out">
                        <td class="whitespace-nowrap p-2 text-sm">{{ $codebase->repository }}</td>
                        <td class="whitespace-nowrap p-2 text-sm">{{ $codebase->branch }}</td>
                        <td class="whitespace-nowrap p-2 text-sm">{{ $codebase->remote }}</td>

                        <td class="whitespace-nowrap p-2 text-sm">{{ $codebase->status }}</td>
                        <td class="whitespace-nowrap p-2 text-sm">{{ $codebase->files_processed }}
                            / {{ $codebase->num_files }}</td>
                        <td class="whitespace-nowrap p-2 text-sm">{{ substr($codebase->sha, 0, 7) }}</td>
                        <td class="whitespace-nowrap p-2 text-sm">{{ $codebase->created_at }}</td>
                    </tr>
                @endforeach
                </tbody>
            </table>
        </div>
    </div>
</div>
