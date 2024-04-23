<div>
    <div class="p-4 max-w-6xl mx-auto">
        <div class="mt-36 grid grid-cols-1 gap-4 items-start">
            <x-pane title="Recent Payments">
                <div class="space-y-2">
                    <div class="grid grid-cols-5 gap-x-4 font-bold">
                        <div>Date</div>
                        <div>ID</div>
                        <div>Recipient</div>
                        <div>Amount (₿)</div>
                        <div>Status</div>
                    </div>
                    @foreach ($prismSinglePayments as $payment)
                        <div class="grid grid-cols-5 gap-x-4">
                            <div>{{ $payment['created_at'] }}</div>
                            <div>{{ Str::limit($payment['id'], 8, '...') }}</div>
                            <div>{{ $payment['receiver_id'] }}</div>
                            <div>{{ number_format($payment['amount_msat'] / 1000) }}</div>
                            <div>{{ $payment['status'] }}</div>
                        </div>
                    @endforeach
                </div>
            </x-pane>
        </div>
    </div>
</div>
