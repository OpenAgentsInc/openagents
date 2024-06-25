<?php

namespace App\Livewire\Modals;

use App\Models\Payin;
use BaconQrCode\Renderer\Image\SvgImageBackEnd;
use BaconQrCode\Renderer\ImageRenderer;
use BaconQrCode\Renderer\RendererStyle\RendererStyle;
use BaconQrCode\Writer;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use LivewireUI\Modal\ModalComponent;

class Invoice extends ModalComponent
{
    use LivewireAlert;

    public $invoice;

    public $qrCode;

    public static function modalMaxWidth(): string
    {
        return '3xl';
    }

    public function render()
    {
        return view('livewire.modals.invoice');
    }

    public function mount($invoice = null)
    {
        $this->invoice = $invoice;

        $renderer = new ImageRenderer(
            new RendererStyle(400),
            new SvgImageBackEnd()
        );
        $writer = new Writer($renderer);
        $this->qrCode = $writer->writeString($this->invoice);
    }

    public function checkInvoiceStatus()
    {

        $payin = Payin::where('payment_request', $this->invoice)->first();
        if (! $payin) {
            $this->alert('error', 'Invoice not found');

            return;
        }
        if ($payin->status == 'settled') {
            $this->alert('success', 'Payment received');
            $this->closeModal();
        }
    }
}
