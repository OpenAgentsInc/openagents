<?php

use Spatie\PdfToText\Pdf;

test('can ingest pdf', function () {

    // Given a PDF, extract the text
    $fileText = Pdf::getText(
        storage_path('app/uploads/0sYtEObUcMCnLo8zAwv7i0cJPfoWTPO4tW7ZblS0.pdf'),
        config('services.pdftotext.path')
    );

    dd($fileText);

});




// Convert into chunks
// Create embeddings for each chunk
// Store the embeddings in a database

// Given a query
// Convert query into embedding
// Find nearest neighbors
// Return results
