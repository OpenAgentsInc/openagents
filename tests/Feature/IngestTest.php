<?php

use Spatie\PdfToText\Pdf;

test('can ingest pdf', function () {

    // Given a PDF, extract the text
    $fileText = Pdf::getText(
        storage_path('app/Portunus.pdf'),
        config('services.pdftotext.path')
    );

    // assert fileText is a string
    expect($fileText)->toBeString();
    // dd($fileText);
});
