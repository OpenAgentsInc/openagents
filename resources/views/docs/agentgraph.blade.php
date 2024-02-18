<x-blank-layout>

    <h1>AgentGraph</h1>
    <p class="mt-2 text-xl text-gray">AgentGraph is a visual scripting language for AI agent workflows.</p>

    <x-graph>
        <x-node id="1" x="100" y="100" title="Start" />
        <x-node id="2" x="600" y="100" title="End" />
        <x-edge :from="['x' => 100, 'y' => 100]" :to="['x' => 600, 'y' => 100]" />
    </x-graph>

</x-blank-layout>
