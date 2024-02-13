<x-app-layout>
    <div x-data="agentBuilder()" x-init="init()" x-on:add-block.window="addBlock($event.detail)">
        <main class="lg:pl-20">
            <div class="xl:pl-96 m-12">
                <div class="font-bold text-lg">{{ $agent->name }}</div>
                <div class="mt-1 text-sm text-gray">{{ $agent->description }}</div>


                <div class="mt-4">
    <template x-for="task in selectedBlocks" :key="task.uniqueKey">
        <div>
<button
@click="runTask(task)"
class="float-right text-white px-4 py-2 rounded-lg mt-2 mr-20">Run Task</button>
            <!-- Task Name and Description -->
            <div class="py-4 mb-4">
                <div class="mb-4">
                    <span class="uppercase text-xs opacity-75 tracking-wider">Task</span>
                    <h2 class="-mt-2 py-2 text-lg font-bold rounded-t-lg" x-text="task.name"></h2>
                    <p x-text="task.description"></p>
                </div>

                <!-- Steps for the Task -->
                <div class="w-full mt-6 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
<template x-for="step in task.steps" :key="step.id">
    <div class="p-4 mb-2 border border-offblack rounded max-w-xs">
        <!-- Step Name -->
        <h3 class="text-normal font-semibold" x-text="`${step.name}`"></h3>
        <!-- Step Description -->
        <p x-text="step.description" class="mt-1 text-sm text-gray"></p>
        <!-- Input for User to Enter Data -->
        <x-input x-model="step.userInput" type="text" placeholder="Enter input here" class="mt-2 mb-2 text-gray-700 text-xs p-1 rounded w-full" />
        <!-- Test Button -->
        <button @click="testBlock(step)" class="mt-2 text-white bg-blue-500 hover:bg-blue-700 text-xs font-bold py-2 px-4 rounded">Test</button>
        <!-- Remove Button -->
        <button @click="removeStep(task, step)" class="mt-2 text-white bg-red-500 hover:bg-red-700 text-xs font-bold py-2 px-4 rounded">Remove</button>
        <!-- Placeholder for Output -->
        <div :id="`output-${step.id}`" class="mt-4 text-sm text-gray"></div>
    </div>
</template>
                </div>
            </div>
        </div>
    </template>
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

<script type="application/json" id="tasksData">
    {!! $tasks->toJson() !!}
</script>


    <script>
        function agentBuilder() {
            return {
                availableBlocks: [],
                selectedBlocks: [],
                init() {
                            const tasksDataElement = document.getElementById('tasksData');
            if (tasksDataElement) {
                const tasksData = JSON.parse(tasksDataElement.textContent);
                console.log('Tasks data:', tasksData);
                this.selectedBlocks = tasksData.map(task => ({
                    ...task,
                    steps: task.steps.sort((a, b) => a.order - b.order),
                    uniqueKey: Date.now() + Math.random() // Ensure each task has a unique key
                }));
            }
        },
                addBlock(block) {
                    let newBlock = JSON.parse(JSON.stringify(block));
                    newBlock.uniqueKey = Date.now() + Math.random();
                    this.selectedBlocks.push(newBlock);
                },
                removeBlock(index) {
                    this.selectedBlocks.splice(index, 1);
                },
 runTask(task) {
            console.log('Running task:', task);
            fetch(`/agent/${task.id}/run`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    "X-CSRF-TOKEN": document.querySelector('meta[name="csrf-token"]').getAttribute('content') // Ensure CSRF token is sent
                },
                body: JSON.stringify({
                    input: "Does this work? https://raw.githubusercontent.com/OpenAgentsInc/plugin-url-scraper/main/src/lib.rs"
                    // include any necessary data here
                })
            })
            .then(response => response.json())
            .then(data => {
                console.log('Task run response:', data);
                // Update the DOM or component state as necessary
                const outputContainer = document.querySelector(`#output-${task.id}`);
                if (outputContainer) {
                    outputContainer.innerHTML = data.output; // Assuming 'data.message' contains the response you want to display
                }
            })
            .catch(error => {
                console.error('Error running task:', error);
            });
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
        // Use the correct ID to select the output container
        const outputContainer = document.querySelector(`#output-${block.id}`);
        if (outputContainer) {
            outputContainer.innerHTML = data.output;
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
