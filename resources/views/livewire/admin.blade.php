<div class="p-12">
    <h2 class="mb-4">Users ({{ $totalUsers }})</h2>

    <div class="flex flex-col gap-8">
        <table class="min-w-full table-fixed divide-y divide-gray text-white">
            <thead>
            <tr>
                <th class="p-3 text-left text-sm font-semibold text-gray">
                    <div>ID</div>
                </th>
                <th class="p-3 text-left text-sm font-semibold text-gray">
                    <div>Name</div>
                </th>
                <th class="p-3 text-left text-sm font-semibold text-gray">
                    <div>Username</div>
                </th>
                <th class="p-3 text-left text-sm font-semibold text-gray">
                    <div># Messages</div>
                </th>
            </tr>
            </thead>
            <tbody class="divide-y divide-gray bg-black text-gray">
            @foreach($users as $user)
                <tr wire:key="{{ $user->id }}">
                    <td class="whitespace-nowrap p-3 text-sm">{{ $user->id }}</td>
                    <td class="whitespace-nowrap p-3 text-sm">{{ $user->name }}</td>
                    <td class="whitespace-nowrap p-3 text-sm">{{ $user->username }}</td>
                    <td class="whitespace-nowrap p-3 text-sm">{{ $user->messages_count }}</td>
                </tr>
            @endforeach
            </tbody>
        </table>
    </div>


    {{--    <table>--}}
    {{--        <thead>--}}
    {{--        <tr>--}}
    {{--            <th>User</th>--}}
    {{--            <th>Messages</th>--}}
    {{--        </tr>--}}
    {{--        </thead>--}}
    {{--        <tbody>--}}
    {{--        @foreach($users as $user)--}}
    {{--            <tr>--}}
    {{--                <td>{{ $user->name }}</td>--}}
    {{--                <td>{{ $user->messages_count }}</td>--}}
    {{--            </tr>--}}
    {{--        @endforeach--}}
    {{--        </tbody>--}}
    {{--    </table>--}}
</div>
