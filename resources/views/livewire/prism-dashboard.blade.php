<div>
    <livewire:navbar/>

    <div class="p-4">

        <div class="mt-12">
            <x-pane title="Funds Overview">
                <div class="flex justify-between"><span>Total:</span> <span>100,000 SAT</span></div>
                <div class="flex justify-between"><span>Used:</span> <span>60,000 SAT (60%)</span></div>
                <div class="flex justify-between"><span>Available:</span> <span>40,000 SAT (40%)</span></div>
                <div class="flex justify-between"><span>Reserved:</span> <span>10,000 SAT</span></div>
                <div class="flex justify-between"><span>Cache:</span> <span>5,000 SAT</span></div>
            </x-pane>

            <x-pane title="Recent Transactions">
                <div class="space-y-2">
                    <div class="grid grid-cols-4 gap-x-4">
                        <div class="col-span-1">ID</div>
                        <div class="col-span-1">User</div>
                        <div class="col-span-1">Amount</div>
                        <div class="col-span-1">Status</div>
                    </div>
                    <div class="grid grid-cols-4 gap-x-4">
                        <div class="col-span-1">12345...</div>
                        <div class="col-span-1">John Doe</div>
                        <div class="col-span-1">500 SAT</div>
                        <div class="col-span-1">Completed</div>
                    </div>
                    <div class="grid grid-cols-4 gap-x-4">
                        <div class="col-span-1">12346...</div>
                        <div class="col-span-1">Jane Smith</div>
                        <div class="col-span-1">1000 SAT</div>
                        <div class="col-span-1">Pending</div>
                    </div>
                </div>
            </x-pane>


            <pre class="mt-4">
┌ CPU - Payment Processing Load ───────────┐┌ mem - Funds Overview ───────────┐
│ Load Average: 1.2 1.5 1.3                ││ Total: 100,000 SAT              │
│ Active Threads: 4                        ││ Used: 60,000 SAT (60%)          │
└──────────────────────────────────────────┘│ Available: 40,000 SAT (40%)     │
                                            │ Reserved: 10,000 SAT            │
┌ disks - Transaction Logs ────────────────┐│ Cache: 5,000 SAT                │
│ Today: 150 TXs Read, 120 TXs Written     │└─────────────────────────────────┘
│ Errors: 3                                │
└──────────────────────────────────────────┘
┌ net - Payment Traffic ───────────────────┐
│ Incoming: 5 TXs/s                        │
│ Outgoing: 3 TXs/s                        │
│ Total TXs: 350 today                     │
└──────────────────────────────────────────┘
┌ procfilter - Recent Transactions ────────┐
│ ID       User        Amount   Status     │
│ 12345... John Doe    500 SAT  Completed  │
│ 12346... Jane Smith  1000 SAT Pending    │
└──────────────────────────────────────────┘
┌ reversentree - Payment Batching ─────────┐
│ Batch ID: XYZ123                         │
│ ├─ John Doe      500 SAT                 │
│ ├─ Jane Smith    1000 SAT                │
│ └─ Acme Corp     2000 SAT                │
└──────────────────────────────────────────┘
┌ sys - System Status ─────────────────────┐
│ API Uptime: 99.97%                       │
│ Server Health: Good                      │
│ Error Rate: 0.1%                         │
└──────────────────────────────────────────┘
            </pre>
        </div>
    </div>
</div>
