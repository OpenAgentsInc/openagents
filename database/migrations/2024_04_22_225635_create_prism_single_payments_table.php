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
        Schema::create('prism_single_payments', function (Blueprint $table) {
            $table->id();
            $table->string('prism_id'); // prismId from the response
            $table->string('payment_id'); // id from the payments array
            //            $table->integer('created_at'); // createdAt from the response
            //            $table->integer('updated_at')->nullable(); // updatedAt from the response
            $table->integer('expires_at')->nullable(); // expiresAt from the response
            $table->string('sender_id'); // senderId from the response
            $table->string('receiver_id'); // receiverId from the response
            $table->integer('amount_msat'); // amountMsat from the response
            $table->string('status'); // status from the response
            $table->integer('resolved_at')->nullable(); // resolvedAt from the response
            $table->boolean('resolved'); // resolved from the response
            $table->string('prism_payment_id')->nullable(); // prismPaymentId from the response
            $table->string('bolt11')->nullable(); // bolt11 from the response
            $table->string('preimage')->nullable(); // preimage from the response
            $table->string('failure_code')->nullable(); // failureCode from the response
            $table->string('type'); // type from the response
            $table->string('reason')->nullable(); // reason from the response
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('prism_single_payments');
    }
};
