<x-app-layout>
    <div x-data="agentBuilder()" x-init="init()" x-on:add-block.window="addBlock($event.detail)">
        <main class="lg:pl-20">
            <div class="xl:pl-96 m-12">
                <div class="font-bold text-lg">{{ $agent->name }}</div>
                <div class="mt-1 text-sm text-gray">{{ $agent->description }}</div>

                <button class="float-right text-white px-4 py-2 rounded-lg -mt-12">Run Flow</button>

                @foreach($agent->tasks as $task)
                    <div class="py-4 mb-4">
                        <div class="mb-4">
                            <span class="uppercase text-xs opacity-75 tracking-wider">Task</span>
                            <h2 class="-mt-2 py-2 text-lg font-bold rounded-t-lg">{{ $task->name }}</h2>
                        </div>
                        <div class="mt-6 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
                            @forelse($task->steps->sortBy('order') as $step)
                                <div class="p-4 border border-offblack rounded">
                                    <h3 class="text-normal font-semibold">
                                        <span>{{ $step->name }}</span>
                                    </h3>
                                    <p>{{ $step->description }}</p>
                                </div>
                            @empty
                                <p class="col-span-full">No steps available for this task.</p>
                            @endforelse
                        </div>
                    </div>
                @endforeach

                <div class="mt-4 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                    <template x-for="(block, index) in selectedBlocks" :key="block.uniqueKey">
                        <div class="p-4 mb-2 border border-offblack rounded max-w-xs">
                            <h3 class="text-normal font-semibold">
                                <span x-text="`${index + 1}. ${block.name}`"></span>
                            </h3>
                            <p x-text="block.description" class="mt-1 text-sm text-gray"></p>
                            <x-input x-model="block.userInput" type="text" placeholder="Enter input here" class="mt-2 mb-2 text-gray-700 text-xs p-1 rounded" />
                            <button @click="testBlock(block)" class="mt-2 text-gray text-xs">Test</button>
                            <button @click="removeBlock(index)" class="mt-2 text-gray text-xs">Remove</button>
                            <div :data-output-id="`output-${block.uniqueKey}`" class="mt-4"></div>
                        </div>
                    </template>
                </div>
            </div>
        </main>

        <aside class="fixed inset-y-0 mt-[64px] left-20 hidden w-96 overflow-y-auto border-r border-offblack px-4 py-6 sm:px-6 lg:px-8 xl:block">
            <div class="px-4 py-10 sm:px-6 lg:px-8 lg:py-2">
                <h1 class="font-bold">Blocks</h1>
                <p class="pb-6 text-gray text-sm mt-1">Click a block to add it to your agent</p>

                <x-input type="text" class="mb-6 w-full" placeholder="Search blocks..." />

                <h2 class="mb-4 font-medium">Plugins</h2>

                <div class="grid grid-cols-1 gap-6 mb-6">
                    @forelse($plugins as $plugin)
                        <x-plugin :plugin="$plugin" />
                    @empty
                        <p class="col-span-full">No plugins available.</p>
                    @endforelse
                </div>

                <h2 class="my-4 font-medium">Parse Blocks</h2>

                <div class="cursor-pointer font-mono border border-offblack rounded-lg p-4">
                    <p class="font-bold text-normal">Array String Loop</p>
                    <p class="mt-2 text-sm text-gray">Loop through an array of strings</p>
                </div>
            </div>
        </aside>
    </div>

    <script>
        function agentBuilder() {
            return {
                availableBlocks: [],
                selectedBlocks: [],
                init() {},
                addBlock(block) {
                    let newBlock = JSON.parse(JSON.stringify(block));
                    newBlock.uniqueKey = Date.now() + Math.random();
                    this.selectedBlocks.push(newBlock);
                },
                removeBlock(index) {
                    this.selectedBlocks.splice(index, 1);
                },
                testBlock(block) {
                    console.log('Testing block:', block);
                    const pluginId = block.id;
                    console.log('Plugin ID:', pluginId);
                    const userInput = block.userInput || 'Default input if empty';
                    fetch("/plugins/call", {
                        method: "POST",
                        headers: {
                            "Content-Type": "application/json",
                            "X-CSRF-TOKEN": "{{ csrf_token() }}"
                        },
                        body: JSON.stringify({
                            plugin_id: pluginId,
                            input: userInput
                        })
                    })
                        .then(response => response.json())
                        .then(data => {
                            console.log(data);
                            const outputContainer = document.querySelector(`[data-output-id='output-${block.uniqueKey}']`);
                            if (outputContainer) {
                                outputContainer.innerHTML = data.output;
                            } else {
                                const newOutputContainer = document.createElement('div');
                                newOutputContainer.setAttribute('id', `output-${block.uniqueKey}`);
                                newOutputContainer.innerHTML = data.output;
                                document.querySelector(`[data-block-id="${block.uniqueKey}"]`).appendChild(newOutputContainer);
                            }
                        })
                        .catch(error => {
                            console.error('Error:', error);
                        });
                }
            }
        }
    </script>

</x-app-layout>
