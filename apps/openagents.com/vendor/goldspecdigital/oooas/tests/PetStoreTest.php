<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests;

use GoldSpecDigital\ObjectOrientedOAS\Objects\AllOf;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Components;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Contact;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Info;
use GoldSpecDigital\ObjectOrientedOAS\Objects\License;
use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Operation;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\PathItem;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Server;
use GoldSpecDigital\ObjectOrientedOAS\OpenApi;

class PetStoreTest extends TestCase
{
    /** @test */
    public function pet_store_example()
    {
        $contact = Contact::create()
            ->name('Swagger API Team')
            ->email('apiteam@swagger.io')
            ->url('http://swagger.io');

        $license = License::create()
            ->name('Apache 2.0')
            ->url('https://www.apache.org/licenses/LICENSE-2.0.html');

        $server = Server::create()
            ->url('http://petstore.swagger.io/api');

        $info = Info::create()
            ->version('1.0.0')
            ->title('Swagger Petstore')
            ->description('A sample API that uses a petstore as an example to demonstrate features in the OpenAPI 3.0 specification')
            ->termsOfService('http://swagger.io/terms/')
            ->contact($contact)
            ->license($license);

        $tagsParameter = Parameter::query()
            ->name('tags')
            ->description('tags to filter by')
            ->required(false)
            ->style(Parameter::STYLE_FORM)
            ->schema(
                Schema::array()->items(
                    Schema::string()
                )
            );

        $limitParameter = Parameter::query()
            ->name('limit')
            ->description('maximum number of results to return')
            ->required(false)
            ->schema(
                Schema::integer()->format(Schema::FORMAT_INT32)
            );

        $petSchema = AllOf::create('Pet')
            ->schemas(
                Schema::ref('#/components/schemas/NewPet'),
                Schema::create()
                    ->required('id')
                    ->properties(
                        Schema::integer('id')->format(Schema::FORMAT_INT64)
                    )
            );

        $newPetSchema = Schema::create('NewPet')
            ->required('name')
            ->properties(
                Schema::string('name'),
                Schema::string('tag')
            );

        $errorSchema = Schema::create('Error')
            ->required('code', 'message')
            ->properties(
                Schema::integer('code')->format(Schema::FORMAT_INT32),
                Schema::string('message')
            );

        $components = Components::create()
            ->schemas($petSchema, $newPetSchema, $errorSchema);

        $petResponse = Response::ok()
            ->description('pet response')
            ->content(
                MediaType::json()->schema(
                    Schema::ref('#/components/schemas/Pet')
                )
            );

        $petListingResponse = Response::ok()
            ->description('pet response')
            ->content(
                MediaType::json()->schema(
                    Schema::array()->items(
                        Schema::ref('#/components/schemas/Pet')
                    )
                )
            );

        $defaultErrorResponse = Response::create('Error')
            ->description('unexpected error')
            ->content(MediaType::json()->schema(
                Schema::ref('#/components/schemas/Error')
            ));

        $findPets = Operation::get()
            ->description("Returns all pets from the system that the user has access to\nNam sed condimentum est. Maecenas tempor sagittis sapien, nec rhoncus sem sagittis sit amet. Aenean at gravida augue, ac iaculis sem. Curabitur odio lorem, ornare eget elementum nec, cursus id lectus. Duis mi turpis, pulvinar ac eros ac, tincidunt varius justo. In hac habitasse platea dictumst. Integer at adipiscing ante, a sagittis ligula. Aenean pharetra tempor ante molestie imperdiet. Vivamus id aliquam diam. Cras quis velit non tortor eleifend sagittis. Praesent at enim pharetra urna volutpat venenatis eget eget mauris. In eleifend fermentum facilisis. Praesent enim enim, gravida ac sodales sed, placerat id erat. Suspendisse lacus dolor, consectetur non augue vel, vehicula interdum libero. Morbi euismod sagittis libero sed lacinia.\n\nSed tempus felis lobortis leo pulvinar rutrum. Nam mattis velit nisl, eu condimentum ligula luctus nec. Phasellus semper velit eget aliquet faucibus. In a mattis elit. Phasellus vel urna viverra, condimentum lorem id, rhoncus nibh. Ut pellentesque posuere elementum. Sed a varius odio. Morbi rhoncus ligula libero, vel eleifend nunc tristique vitae. Fusce et sem dui. Aenean nec scelerisque tortor. Fusce malesuada accumsan magna vel tempus. Quisque mollis felis eu dolor tristique, sit amet auctor felis gravida. Sed libero lorem, molestie sed nisl in, accumsan tempor nisi. Fusce sollicitudin massa ut lacinia mattis. Sed vel eleifend lorem. Pellentesque vitae felis pretium, pulvinar elit eu, euismod sapien.\n")
            ->operationId('findPets')
            ->parameters($tagsParameter, $limitParameter)
            ->responses($petListingResponse, $defaultErrorResponse);

        $addPet = Operation::post()
            ->description('Creates a new pet in the store.  Duplicates are allowed')
            ->operationId('addPet')
            ->requestBody(
                RequestBody::create()
                    ->description('Pet to add to the store')
                    ->required()
                    ->content(
                        MediaType::json()->schema(
                            Schema::ref('#/components/schemas/NewPet')
                        )
                    )
            )
            ->responses($petResponse, $defaultErrorResponse);

        $petRoot = PathItem::create()
            ->route('/pets')
            ->operations($findPets, $addPet);

        $petIdParameter = Parameter::path()
            ->name('id')
            ->description('ID of pet to fetch')
            ->required()
            ->schema(
                Schema::integer()->format(Schema::FORMAT_INT64)
            );

        $findPetById = Operation::get()
            ->description('Returns a user based on a single ID, if the user does not have access to the pet')
            ->operationId('find pet by id')
            ->parameters($petIdParameter)
            ->responses($petResponse, $defaultErrorResponse);

        $petDeletedResponse = Response::create()
            ->statusCode(204)
            ->description('pet deleted');

        $deletePetById = Operation::delete()
            ->description('deletes a single pet based on the ID supplied')
            ->operationId('deletePet')
            ->parameters($petIdParameter->description('ID of pet to delete'))
            ->responses($petDeletedResponse, $defaultErrorResponse);

        $petNested = PathItem::create()
            ->route('/pets/{id}')
            ->operations($findPetById, $deletePetById);

        $openApi = OpenApi::create()
            ->openapi(OpenApi::OPENAPI_3_0_0)
            ->info($info)
            ->servers($server)
            ->paths($petRoot, $petNested)
            ->components($components);

        $exampleResponse = file_get_contents(realpath(__DIR__) . '/storage/petstore_expanded.json');

        $this->assertEquals(
            json_decode($exampleResponse, true),
            $openApi->toArray()
        );
    }
}
