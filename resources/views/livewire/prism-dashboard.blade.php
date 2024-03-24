<div>
    <livewire:navbar/>

    <div class="p-4">
        <div class="mt-12 font-bold text-lg">Prism payments</div>
        <div class="mt-4">
            <x-pane/>


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
