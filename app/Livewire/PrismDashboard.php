<?php

namespace App\Livewire;

use App\Models\Payment;
use Carbon\Carbon;
use Livewire\Component;

class PrismDashboard extends Component
{
    public $payments;

    protected $listeners = ['echo:payments,PaymentCreated' => 'ohshit'];

    public function ohshit()
    {
        dd('NICE');
    }

    public function mount()
    {
        // Check if there are any payments in the database
        if (Payment::count() > 0) {
            $this->payments = Payment::all()->map(function ($payment) {
                // Assuming you want to keep the same structure as your hardcoded data
                return [
                    'id' => $payment->prism_id,
                    'createdAt' => Carbon::createFromTimestamp($payment->prism_created_at)->format('Y-m-d H:i:s'),
                    'updatedAt' => Carbon::createFromTimestamp($payment->prism_updated_at)->format('Y-m-d H:i:s'),
                    'expiresAt' => Carbon::createFromTimestamp($payment->expires_at)->format('Y-m-d H:i:s'),
                    'senderId' => $payment->sender_prism_id,
                    'receiverId' => $payment->receiver_prism_id,
                    'receiverAddress' => $payment->receiver->ln_address ?? 'unknown_demo@blah.com', // Example, adjust based on your model relations
                    'amountMsat' => $payment->amount_msat,
                    'status' => $payment->status,
                    'resolvedAt' => $payment->resolved_at ? Carbon::createFromTimestamp($payment->resolved_at)->format('Y-m-d H:i:s') : null,
                    'resolved' => $payment->resolved,
                    'prismPaymentId' => $payment->prism_payment_id,
                    'bolt11' => $payment->bolt11,
                    'preimage' => $payment->preimage,
                    'failureCode' => $payment->failure_code,
                    'type' => $payment->type,
                ];
            })->toArray();
        } else {

            $this->payments = [
                [
                    'id' => '1131a470-e36e-4613-b6fb-b94a6f46d608',
                    'createdAt' => 1711233884,
                    'updatedAt' => 1711233889,
                    'expiresAt' => 1711320285,
                    'senderId' => '68f5d9c3-9260-4fdc-b29f-8e5e8edcb849',
                    'receiverId' => '9a96fe41-30a6-4b84-96a2-7184f107be96',
                    'receiverAddress' => 'mcdonald55@bitnob.io',
                    'amountMsat' => 50000,
                    'status' => 'paid',
                    'resolvedAt' => 1711233889,
                    'resolved' => 1,
                    'prismPaymentId' => '81c1d6dd-a844-493d-b66d-70864f7e2cb3',
                    'bolt11' => 'lnbc500n1pjl7k67pp5qcqj4zr0gd7whqqtq06c9z076wsng3aknqe6uda56ag46wgd3lgqhp56ynfunw42yadks5q4rm43d4p3zvpypax3xavqvc04328psfc6glscqzzsxqyz5tlsp52uwws6xjppf68k74apaty8cgfrftp5vz78rqewskszu08k372psq9qyyssqt8z7gw5w7fl49kwyjxj0h8wu9668t8q74avnzsnkd42djj0qt67ku3mtpqk7d97nn2j9k8gr9sy9erjs233g6jr0vcnq96npj3djhrcpf5nr8m',
                    'preimage' => '519eac0b500c99d3574747400ccf4f219aa618bac847b09a33e0f539124779ee',
                    'failureCode' => '',
                    'type' => 'DEFAULT',
                ],
                [
                    'id' => 'b98fcb73-01a5-45bb-9945-8bc5aa8a6400',
                    'createdAt' => 1711233884,
                    'updatedAt' => 1711233962,
                    'expiresAt' => 1711320284,
                    'senderId' => '68f5d9c3-9260-4fdc-b29f-8e5e8edcb849',
                    'receiverId' => '3cf9098c-283f-4843-8ee8-ada9a907a75f',
                    'receiverAddress' => 'rblb@blink.sv',
                    'amountMsat' => 50000,
                    'status' => 'paid',
                    'resolvedAt' => 1711233962,
                    'resolved' => 1,
                    'prismPaymentId' => '81c1d6dd-a844-493d-b66d-70864f7e2cb3',
                    'bolt11' => 'lnbc500n1pjl7k6upp5zk795fmm54a99jf5ph7hwwkx6rr5cftajv88w08p4ewsh0wr7zhshp5g9wy0djmcjy77pp409def8auqe356lhpxk0kuv8l9p7g8m8694zqcqzpuxqyz5vqsp5tassd0fc9f4nyea020zxk0qgwf7w4xenz05gez5pkzemjvexvy7s9qyyssqp8vczjyrwxe5rxe0mtgszvcm0lhgwr4x7dgyku8gamefheef5k3xgjv8v6yjsfm6dj4cpvf05ax44xxv5m67d84m5w6hcz3al453y9cqm67xvx',
                    'preimage' => '',
                    'failureCode' => '',
                    'type' => 'DEFAULT',
                ],
            ];

            // Convert createdAt timestamps to formatted dates
            foreach ($this->payments as &$payment) {
                $payment['createdAt'] = Carbon::createFromTimestamp($payment['createdAt'])
                    ->format('Y-m-d H:i:s');
            }
        }
    }

    public function render()
    {
        return view('livewire.prism-dashboard');
    }
}
