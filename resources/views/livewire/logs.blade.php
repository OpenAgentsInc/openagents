<div class="p-12">
    <div class="flex flex-row justify-between">
        <h2 class="mb-4">Logs ({{ count($logs) }})</h2>
    </div>

    <div class="flex flex-col gap-8">
        <table class="min-w-full table-fixed divide-y divide-offblack text-white">
            <thead>
            <tr>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    <div>ID</div>
                </th>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    <div>Data</div>
                </th>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    <div>Created At</div>
                </th>
            </tr>
            </thead>
            <tbody class="divide-y divide-offblack bg-black text-gray">
            @foreach($logs as $log)
                <tr>
                    <td class="whitespace-nowrap p-2 text-sm">{{ $log->id }}</td>
                    <td class="whitespace-nowrap p-2 text-sm">{{ json_encode($log->data) }}</td>
                    <td class="whitespace-nowrap p-2 text-sm">{{ $log->created_at }}</td>
                </tr>
            @endforeach
            </tbody>
        </table>
    </div>
</div>
