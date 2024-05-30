<div>
    <x-pane title="Your agents' bitcoin balance">
        <div class="p-3">
            <h4 class="text-center">{{ $satsBalance }} sats</h4>
        </div>
    </x-pane>
    <p class="mt-4 text-sm text-center text-gray">Agent balances are transferred once per minute: 80% to the agent
        creator, 20% to OpenAgents (or when plugins are used, 60% to the agent creator, 20% to the plugin authors, 20%
        to OA)</p>
</div>