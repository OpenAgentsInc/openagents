<?php

namespace App\Livewire\Modals;

use App\Http\Controllers\LnAddressController;
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
        $controller = new LnAddressController();
        $invoiceStatus = $controller->getInvoiceStatus($this->invoice);
        if ($invoiceStatus && $invoiceStatus['settled']) {
            $this->alert('success', 'Payment received');
            $this->closeModal();
        }
    }
}
