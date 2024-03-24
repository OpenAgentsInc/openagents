<div>
    <livewire:navbar/>
    <div class="p-4 max-w-6xl mx-auto">
        <div class="mt-36 grid grid-cols-1 md:grid-cols-2 gap-4 items-start">
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
        </div>
    </div>
</div>
