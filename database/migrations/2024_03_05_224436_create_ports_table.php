<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('ports', function (Blueprint $table) {
            $table->id();

            // Each port belongs to a node.
            $table->foreignId('node_id')->constrained()->cascadeOnDelete();

            // Define whether the port is an input or output port.
            $table->enum('type', ['input', 'output']);

            // Ports can have a name, e.g., "URL Input" or "Data Output".
            $table->string('name');

            // Optional description for what the port does or expects.
            $table->string('description')->nullable();

            // For Initial Information Packets (IIPs), possibly stored as JSON.
            $table->json('initial_data')->nullable();

            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('ports');
    }
};
