<div>
    <livewire:navbar/>
    <div class="p-4 max-w-6xl mx-auto">
        <div class="mt-36 grid grid-cols-1 gap-4 items-start">

            <!--<x-pane title="Funds Overview">
                <div class="flex justify-between"><span>Total:</span> <span>100,000 SAT</span></div>
                <div class="flex justify-between"><span>Used:</span> <span>60,000 SAT (60%)</span></div>
                <div class="flex justify-between"><span>Available:</span> <span>40,000 SAT (40%)</span></div>
                <div class="flex justify-between"><span>Reserved:</span> <span>10,000 SAT</span></div>
                <div class="flex justify-between"><span>Cache:</span> <span>5,000 SAT</span></div>
            </x-pane>
            -->

            <x-pane title="Recent Payments">
                <div class="space-y-2">
                    <div class="grid grid-cols-4 gap-x-4 font-bold">
                        <div>ID</div>
                        <div>User</div>
                        <div>Amount (₿)</div>
                        <div>Status</div>
                    </div>
                    @foreach ($payments as $payment)
                        <div class="grid grid-cols-4 gap-x-4">
                            <div>{{ Str::limit($payment['id'], 8, '...') }}</div>
                            <div>{{ Str::limit($payment['receiverId'], 8, '...') }}</div>
                            <div>{{ number_format($payment['amountMsat'] / 1000) }}</div>
                            <div>{{ $payment['status'] }}</div>
                        </div>
                    @endforeach
                </div>
            </x-pane>
        </div>
    </div>
</div>
