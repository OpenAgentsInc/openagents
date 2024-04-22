<div class="p-12">
    <h2 class="mb-4">Users ({{ $totalUsers }})</h2>

    <div class="flex flex-col gap-8">
        <table class="min-w-full table-fixed divide-y divide-offblack text-white">
            <thead>
            <tr>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    <div>ID</div>
                </th>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    <div>Name</div>
                </th>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    <div>Username</div>
                </th>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    <div># Messages</div>
                </th>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    <div>Pro</div>
                </th>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    <div>Joined</div>
                </th>
                <th class="p-2 text-left text-sm font-semibold text-gray">
                    {{-- Dropdown--}}
                </th>
            </tr>
            </thead>
            <tbody class="divide-y divide-offblack bg-black text-gray">
            @foreach($users as $user)
                <tr wire:key="{{ $user->id }}">
                    <td class="whitespace-nowrap p-2 text-sm">{{ $user->id }}</td>
                    <td class="whitespace-nowrap p-2 text-sm">{{ $user->name }}</td>
                    <td class="whitespace-nowrap p-2 text-sm">{{ $user->username }}</td>
                    <td class="whitespace-nowrap p-2 text-sm">{{ $user->messages_count }}</td>
                    <td class="whitespace-nowrap p-2 text-sm">{{ $user->is_pro ? "Yes" : "No" }}</td>
                    <td class="whitespace-nowrap p-2 text-sm">{{ $user->dateForHumans() }}</td>
                    <td class="whitespace-nowrap p-2 text-sm">
                        <div class="flex items-center justify-end">
                            <div x-data x-menu class="relative">
                                <button x-menu:button
                                        class="flex items-center gap-2 pl-5 pr-3 py-2.5 rounded-md shadow">
                                    <span>...</span>
                                </button>

                                <div
                                        x-menu:items
                                        x-transition.origin.top.right
                                        class="absolute right-0 w-48 mt-2 z-10 origin-top-right bg-black border border-offblack divide-y divide-offblack rounded-md shadow-md py-1 outline-none"
                                        x-cloak
                                >
                                    <a
                                            x-menu:item
                                            href="#edit"
                                            :class="{
                'bg-cyan-500/10 text-gray-900': $menuItem.isActive,
                'text-gray-600': ! $menuItem.isActive,
                'opacity-50 cursor-not-allowed': $menuItem.isDisabled,
            }"
                                            class="block w-full px-4 py-2 text-sm transition-colors"
                                    >
                                        Edit
                                    </a>
                                    <a
                                            x-menu:item
                                            href="#copy"
                                            :class="{
                'bg-cyan-500/10 text-gray-900': $menuItem.isActive,
                'text-gray-600': ! $menuItem.isActive,
                'opacity-50 cursor-not-allowed': $menuItem.isDisabled,
            }"
                                            class="block w-full px-4 py-2 text-sm transition-colors"
                                    >
                                        Copy
                                    </a>
                                    <a
                                            x-menu:item
                                            href="#delete"
                                            :class="{
                'bg-cyan-500/10 text-gray-900': $menuItem.isActive,
                'text-gray-600': ! $menuItem.isActive,
                'opacity-50 cursor-not-allowed': $menuItem.isDisabled,
            }"
                                            class="block w-full px-4 py-2 text-sm transition-colors"
                                            disabled
                                    >
                                        Delete
                                    </a>
                                </div>
                            </div>
                        </div>
                    </td>
                </tr>
            @endforeach
            </tbody>
        </table>
    </div>
</div>
