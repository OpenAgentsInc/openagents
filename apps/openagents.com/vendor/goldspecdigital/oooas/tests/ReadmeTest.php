<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests;

use GoldSpecDigital\ObjectOrientedOAS\Objects\AllOf;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Info;
use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Operation;
use GoldSpecDigital\ObjectOrientedOAS\Objects\PathItem;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Tag;
use GoldSpecDigital\ObjectOrientedOAS\OpenApi;

class ReadmeTest extends TestCase
{
    /** @test */
    public function the_readme_example()
    {
        // Create a tag for all the user endpoints.
        $usersTag = Tag::create()
            ->name('Users')
            ->description('All user related endpoints');

        // Create the info section.
        $info = Info::create()
            ->title('API Specification')
            ->version('v1')
            ->description('For using the Example App API');

        // Create the user schema.
        $userSchema = Schema::object()
            ->properties(
                Schema::string('id')->format(Schema::FORMAT_UUID),
                Schema::string('name'),
                Schema::integer('age')->example(23),
                Schema::string('created_at')->format(Schema::FORMAT_DATE_TIME)
            );

        // Create the user response.
        $userResponse = Response::create()
            ->statusCode(200)
            ->description('OK')
            ->content(
                MediaType::json()->schema($userSchema)
            );

        // Create the operation for the route (i.e. GET, POST, etc.).
        $showUser = Operation::get()
            ->responses($userResponse)
            ->tags($usersTag)
            ->summary('Get an individual user')
            ->operationId('users.show');

        // Define the /users path along with the supported operations.
        $usersPath = PathItem::create()
            ->route('/users')
            ->operations($showUser);

        // Create the main OpenAPI object composed off everything created above.
        $openApi = OpenApi::create()
            ->openapi(OpenApi::OPENAPI_3_0_1)
            ->info($info)
            ->paths($usersPath)
            ->tags($usersTag);

        $readmeExample = file_get_contents(realpath(__DIR__) . '/storage/readme_example.json');

        $this->assertEquals(
            json_decode($readmeExample, true),
            $openApi->toArray()
        );
    }

    /** @test */
    public function setting_and_unsetting_properties()
    {
        $info = Info::create()
            ->title('Example API');

        $openApi = OpenAPI::create()
            ->info($info);

        $this->assertEquals([
            'info' => [
                'title' => 'Example API',
            ],
        ], $openApi->toArray());

        $openApi = $openApi->info(null);

        $this->assertEquals([], $openApi->toArray());
    }

    /** @test */
    public function unsetting_variadic_methods()
    {
        $path = PathItem::create()
            ->route('/users');

        $openApi = OpenAPI::create()
            ->paths($path);

        $this->assertEquals([
            'paths' => [
                '/users' => [],
            ],
        ], $openApi->toArray());

        $openApi = $openApi->paths();

        $this->assertEquals([], $openApi->toArray());
    }

    /** @test */
    public function retrieving_properties()
    {
        $info = Info::create()->title('Example API');

        $this->assertEquals('Example API', $info->title);
    }

    /** @test */
    public function object_id()
    {
        $schema = Schema::create()
            ->type(Schema::TYPE_OBJECT)
            ->properties(
                Schema::create('username')->type(Schema::TYPE_STRING),
                Schema::create('age')->type(Schema::TYPE_INTEGER)
            );

        $this->assertEquals([
            'type' => 'object',
            'properties' => [
                'username' => [
                    'type' => 'string',
                ],
                'age' => [
                    'type' => 'integer',
                ],
            ],
        ], $schema->toArray());
    }

    /** @test */
    public function simpler_object_id()
    {
        $schema = Schema::object()
            ->properties(
                Schema::string('username'),
                Schema::integer('age')
            );

        $this->assertEquals([
            'type' => 'object',
            'properties' => [
                'username' => [
                    'type' => 'string',
                ],
                'age' => [
                    'type' => 'integer',
                ],
            ],
        ], $schema->toArray());
    }

    /** @test */
    public function dollar_ref()
    {
        $schema = AllOf::create()
            ->schemas(
                Schema::ref('#/components/schemas/ExampleSchema')
            );

        $this->assertEquals([
            'allOf' => [
                ['$ref' => '#/components/schemas/ExampleSchema'],
            ],
        ], $schema->toArray());
    }
}
