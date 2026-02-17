<?php

namespace App\OpenApi\RequestBodies;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\RequestBodyFactory;

class UpdateAutopilotRequestBody extends RequestBodyFactory
{
    public function build(): RequestBody
    {
        return RequestBody::create()
            ->description('Update autopilot fields and optional profile/policy blocks.')
            ->required()
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::string('displayName')->nullable()->maxLength(120)->example('Autopilot'),
                        Schema::string('status')->nullable()->example('active'),
                        Schema::string('visibility')->nullable()->example('private'),
                        Schema::string('avatar')->nullable()->maxLength(255),
                        Schema::string('tagline')->nullable()->maxLength(255),
                        Schema::object('profile')->nullable()->properties(
                            Schema::string('ownerDisplayName')->nullable()->maxLength(120),
                            Schema::string('personaSummary')->nullable(),
                            Schema::string('autopilotVoice')->nullable()->maxLength(64),
                            Schema::array('principles')->items(Schema::string()),
                            Schema::object('preferences')->additionalProperties(Schema::string()),
                            Schema::object('onboardingAnswers')->additionalProperties(Schema::string()),
                            Schema::integer('schemaVersion')->nullable(),
                        ),
                        Schema::object('policy')->nullable()->properties(
                            Schema::string('modelProvider')->nullable()->maxLength(64),
                            Schema::string('model')->nullable()->maxLength(128),
                            Schema::array('toolAllowlist')->items(Schema::string()),
                            Schema::array('toolDenylist')->items(Schema::string()),
                            Schema::boolean('l402RequireApproval')->nullable(),
                            Schema::integer('l402MaxSpendMsatsPerCall')->nullable(),
                            Schema::integer('l402MaxSpendMsatsPerDay')->nullable(),
                            Schema::array('l402AllowedHosts')->items(Schema::string()),
                            Schema::object('dataPolicy')->additionalProperties(Schema::string()),
                        ),
                    )
                )
            );
    }
}
