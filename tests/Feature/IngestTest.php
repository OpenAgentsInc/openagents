<?php

use Spatie\PdfToText\Pdf;

test('can ingest pdf', function () {

    // Given a PDF, extract the text
    $fileText = Pdf::getText(
        storage_path('app/Portunus.pdf'),
        config('services.pdftotext.path')
    );

    dd($fileText);
});
