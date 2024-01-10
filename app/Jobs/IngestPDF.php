<?php

namespace App\Jobs;

use App\Models\Agent;
use App\Models\Brain;
use App\Models\Datapoint;
use App\Models\File;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldBeUnique;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;
use Illuminate\Support\Str;
use Spatie\PdfToText\Pdf;

class IngestPDF implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    public string $path;
    public Agent $agent;
    public Brain $brain;
    public File $file;

    /**
     * Create a new job instance.
     */
    public function __construct($path, $agent, $brain, $file)
    {
        $this->path = $path;
        $this->agent = $agent;
        $this->brain = $brain;
        $this->file = $file;
    }

    /**
     * Execute the job.i
     */
    public function handle(): void
    {
        try {
            // Given a PDF, extract the text
            $fileText = Pdf::getText(
                storage_path("app/" . $this->path),
                config('services.pdftotext.path')
            );

            // Split into page chunks
            $chunks = Str::of($fileText)
                ->split("/\f/") // Splitting the text into pages based on the form feed character
                ->transform(function ($page) {
                    return (string) Str::of($page)
                        ->replace("\n", " ") // Replacing newline characters with spaces
                        ->trim(); // Trimming leading and trailing whitespace
                })
                ->toArray();

            // Skip embeddings for now

            // For each chunk, create a Datapoint
            foreach ($chunks as $chunk) {
                // Skip empty chunks
                if (strlen($chunk) === 0) {
                    continue;
                }

                CreateDatapointEmbedding::dispatch($chunk, $this->agent, $this->brain, $this->file);
            }
        } catch (\Exception $e) {
            dump($e->getMessage());
        }

    }
}
