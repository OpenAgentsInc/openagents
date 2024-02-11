<x-app-layout>
    <div x-data="agentBuilder()" x-init="init()" x-on:add-block.window="addBlock($event.detail)">
        <div
            class="hidden lg:fixed lg:inset-y-[60px] lg:left-0 lg:z-50 lg:block lg:w-20 lg:overflow-y-auto lg:bg-gray-900 lg:pb-4">
            <nav class="mt-8">
                <ul role="list" class="flex flex-col items-center space-y-1">
                    <li>
                        <a href="#"
                            class="bg-gray-800 text-white group flex gap-x-3 rounded-md p-3 text-sm leading-6 font-semibold">
                            <svg class="h-6 w-6 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5"
                                stroke="currentColor" aria-hidden="true">
                                <path stroke-linecap="round" stroke-linejoin="round"
                                    d="M2.25 12l8.954-8.955c.44-.439 1.152-.439 1.591 0L21.75 12M4.5 9.75v10.125c0 .621.504 1.125 1.125 1.125H9.75v-4.875c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125V21h4.125c.621 0 1.125-.504 1.125-1.125V9.75M8.25 21h8.25" />
                            </svg>
                            <span class="sr-only">Dashboard</span>
                        </a>
                    </li>
                    <li>
                        <a href="#"
                            class="text-gray-400 hover:text-white hover:bg-gray-800 group flex gap-x-3 rounded-md p-3 text-sm leading-6 font-semibold">
                            <svg class="h-6 w-6 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5"
                                stroke="currentColor" aria-hidden="true">
                                <path stroke-linecap="round" stroke-linejoin="round"
                                    d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z" />
                            </svg>
                            <span class="sr-only">Team</span>
                        </a>
                    </li>
                    <li>
                        <a href="#"
                            class="text-gray-400 hover:text-white hover:bg-gray-800 group flex gap-x-3 rounded-md p-3 text-sm leading-6 font-semibold">
                            <svg class="h-6 w-6 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5"
                                stroke="currentColor" aria-hidden="true">
                                <path stroke-linecap="round" stroke-linejoin="round"
                                    d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z" />
                            </svg>
                            <span class="sr-only">Projects</span>
                        </a>
                    </li>
                    <li>
                        <a href="#"
                            class="text-gray-400 hover:text-white hover:bg-gray-800 group flex gap-x-3 rounded-md p-3 text-sm leading-6 font-semibold">
                            <svg class="h-6 w-6 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5"
                                stroke="currentColor" aria-hidden="true">
                                <path stroke-linecap="round" stroke-linejoin="round"
                                    d="M6.75 3v2.25M17.25 3v2.25M3 18.75V7.5a2.25 2.25 0 012.25-2.25h13.5A2.25 2.25 0 0121 7.5v11.25m-18 0A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75m-18 0v-7.5A2.25 2.25 0 015.25 9h13.5A2.25 2.25 0 0121 11.25v7.5" />
                            </svg>
                            <span class="sr-only">Calendar</span>
                        </a>
                    </li>
                    <li>
                        <a href="#"
                            class="text-gray-400 hover:text-white hover:bg-gray-800 group flex gap-x-3 rounded-md p-3 text-sm leading-6 font-semibold">
                            <svg class="h-6 w-6 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5"
                                stroke="currentColor" aria-hidden="true">
                                <path stroke-linecap="round" stroke-linejoin="round"
                                    d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 01-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 011.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 00-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 01-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 00-3.375-3.375h-1.5a1.125 1.125 0 01-1.125-1.125v-1.5a3.375 3.375 0 00-3.375-3.375H9.75" />
                            </svg>
                            <span class="sr-only">Documents</span>
                        </a>
                    </li>
                    <li>
                        <a href="#"
                            class="text-gray-400 hover:text-white hover:bg-gray-800 group flex gap-x-3 rounded-md p-3 text-sm leading-6 font-semibold">
                            <svg class="h-6 w-6 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5"
                                stroke="currentColor" aria-hidden="true">
                                <path stroke-linecap="round" stroke-linejoin="round"
                                    d="M10.5 6a7.5 7.5 0 107.5 7.5h-7.5V6z" />
                                <path stroke-linecap="round" stroke-linejoin="round"
                                    d="M13.5 10.5H21A7.5 7.5 0 0013.5 3v7.5z" />
                            </svg>
                            <span class="sr-only">Reports</span>
                        </a>
                    </li>
                </ul>
            </nav>
        </div>

        <div class="sticky top-[40px] z-40 flex items-center gap-x-6 bg-gray-900 px-4 py-4 shadow-sm sm:px-6 lg:hidden">
            <button type="button" class="-m-2.5 p-2.5 text-gray-400 lg:hidden">
                <span class="sr-only">Open sidebar</span>
                <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"
                    aria-hidden="true">
                    <path stroke-linecap="round" stroke-linejoin="round"
                        d="M3.75 6.75h16.5M3.75 12h16.5m-16.5 5.25h16.5" />
                </svg>
            </button>
            <div class="flex-1 text-sm font-semibold leading-6 text-white">Dashboard</div>
            <a href="#">
                <span class="sr-only">Your profile</span>
                <img class="h-8 w-8 rounded-full bg-gray-800"
                    src="https://images.unsplash.com/photo-1472099645785-5658abf4ff4e?ixlib=rb-1.2.1&ixid=eyJhcHBfaWQiOjEyMDd9&auto=format&fit=facearea&facepad=2&w=256&h=256&q=80"
                    alt="">
            </a>
        </div>

        <main class="lg:pl-20">
            <div class="xl:pl-96 m-12">
                <div class="font-bold text-xl">{{ $agent->name }}</div>
                <div class="mt-1 text-sm text-gray">{{ $agent->description }}</div>

                <!-- Agent Flow Area -->
                <div class="mt-4">
                    <!-- Use template and ensure it contains only one root element -->
                    <template x-for="(block, index) in selectedBlocks" :key="block.uniqueKey">
                        <div class="p-4 mb-2 bg-gray-700 rounded">
                            <h3 x-text="block.name" class="text-xl font-semibold"></h3>
                            <p x-text="block.description" class="mt-1 text-sm text-gray-500"></p>
                            <button @click="removeBlock(index)" class="mt-2 text-red-500">Remove</button>
                        </div>
                    </template>
                </div>
            </div>
        </main>

        <aside
            class="fixed inset-y-0 mt-[64px] left-20 hidden w-96 overflow-y-auto border-r border-offblack px-4 py-6 sm:px-6 lg:px-8 xl:block">
            <!-- Secondary column (hidden on smaller screens) -->
            <div class="px-4 py-10 sm:px-6 lg:px-8 lg:py-2">
                <h1 class="font-bold">Agent Blocks</h1>
                <p class="pb-6 text-gray">Click a block to add it to your agent</p>
                <div class="grid grid-cols-1 gap-6 mb-6">
                    @forelse($plugins as $plugin)
                        <x-plugin :plugin="$plugin" />
                    @empty
                        <p class="col-span-full">No plugins available.</p>
                    @endforelse
                </div>
            </div>
        </aside>
    </div>

    <script>
        function agentBuilder() {
            return {
                availableBlocks: [], // This should be filled with blocks from server
                selectedBlocks: [],
                init() {
                    // Fetch available blocks from server and populate availableBlocks
                    // You might use this.availableBlocks = {{ $plugins->toJson() }} if $plugins is a collection of blocks
                    // For debugging
                },
                addBlock(block) {
                    // Clone the block object to prevent direct reference issues
                    let newBlock = JSON.parse(JSON.stringify(block));

                    // Append a unique identifier to the block
                    newBlock.uniqueKey = Date.now() + Math.random();

                    // Add the new block with a unique key to the selected blocks array
                    this.selectedBlocks.push(newBlock);
                },

                removeBlock(index) {
                    // For debugging
                    // console.log('Removing block at index:', index);
                    this.selectedBlocks.splice(index, 1);
                },
            }
        }

    </script>

</x-app-layout>
