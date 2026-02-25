<p align="center">
    <a href="https://github.com/goldspecdigital/oooas"><img 
        alt="Object Oriented OpenAPI Specification"
        src="https://svgshare.com/i/D70.svg" width="400px"
    ></a>
</p>

<p align="center">
    <a href="https://github.com/goldspecdigital/oooas"><img 
        alt="GitHub stars" 
        src="https://img.shields.io/github/stars/goldspecdigital/oooas.svg?style=social"
    ></a>
</p>

<p align="center">
    <a href="https://github.com/goldspecdigital/oooas/tags"><img 
        alt="GitHub tag (latest SemVer)" 
        src="https://img.shields.io/github/tag/goldspecdigital/oooas.svg"
    ></a>
    <a href="https://github.com/goldspecdigital/oooas/actions"><img 
        alt="Build status"
        src="https://github.com/goldspecdigital/oooas/workflows/Tests/badge.svg" 
    ></a>
    <a href="https://packagist.org/packages/goldspecdigital/oooas"><img 
        alt="Packagist" 
        src="https://img.shields.io/packagist/dt/goldspecdigital/oooas.svg"
    ></a>
    <img 
        alt="PHP from Packagist" 
        src="https://img.shields.io/packagist/php-v/goldspecdigital/oooas.svg"
    >
    <img 
        alt="Dependency count"
        src="https://img.shields.io/badge/dependencies-0-brightgreen.svg" 
    >
    <img 
        alt="Packagist" 
        src="https://img.shields.io/packagist/l/goldspecdigital/oooas.svg"
    >
</p>

## Introduction

An object oriented approach to generating OpenAPI specs, implemented in PHP. 

You can build up your API spec using immutable PHP classes, and then export the 
spec to JSON (or YAML with the help of another package).

This package is **dependency free** and makes heavy use of **PHP 7 features**, 
mainly being **type hints** and enabling **strict types**. This should make your 
life a lot easier when working with a good IDE that can use this information.

## Installing

You can install the package via composer:
```bash
composer require goldspecdigital/oooas
```

## Example

See the code sample below for the most basic usage:

```php
use GoldSpecDigital\ObjectOrientedOAS\Objects\{
    Info, MediaType, Operation, PathItem, Response, Schema, Tag
};
use GoldSpecDigital\ObjectOrientedOAS\OpenApi;

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
    ->openapi(OpenApi::OPENAPI_3_0_2)
    ->info($info)
    ->paths($usersPath)
    ->tags($usersTag);
    
header('Content-Type: application/json');
echo $openApi->toJson();
```

### YAML output

Using the same code above will output the following YAML:

> In this example, the YAML may seem simpler to look at, however once the spec
starts to increase in size - the ability to reuse objects and split them into
separate files easily will be a massive help.

```yaml
openapi: 3.0.2
info:
  title: API Specification
  description: For using the Example App API
  version: v1
paths:
  "/users":
    get:
      tags:
      - Users
      summary: Get an individual user
      operationId: users.show
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  id:
                    format: uuid
                    type: string
                  name:
                    type: string
                  age:
                    type: integer
                    example: 23
                  created_at:
                    format: date-time
                    type: string
tags:
- name: Users
  description: All user related endpoints
```

### Outputting as JSON or YAML

Built in output to YAML has been omitted on purpose to keep this package
dependency free. However, you can easily convert the array to a YAML string 
using several open source packages. See below for an example of  outputting to 
both JSON and YAML:

```php
use GoldSpecDigital\ObjectOrientedOAS\OpenApi;
use Symfony\Component\Yaml\Yaml;

$openApi = OpenApi::create();

$json = $openApi->toJson();
$array = $openApi->toArray();
$yaml = Yaml::dump($array);
```

## Guidance

If you want to learn more about the OpenAPI schema, then have a look at the 
official [OpenAPI Specification](https://github.com/OAI/OpenAPI-Specification/blob/master/versions/3.0.2.md).

Alternatively, if you would like a quick reference, then check out the 
[OpenAPI Map](https://openapi-map.apihandyman.io/?version=3.0) project created 
by [Arnaud Lauret](http://apihandyman.io/).

You can use this interactive tool to figure out what objects go where and how
they relate to one another.

## Usage

### Setting and unsetting properties

Each object has setter methods for it's supported properties. Most of these 
methods allow `null` values which will need to be explicitly passed (see the 
next example for how to unset using variadic setter methods). This will have the 
effect of unsetting the property:

```php
$info = Info::create()
    ->title('Example API');

$openApi = OpenApi::create()
    ->info($info);
echo $openApi->toJson(); // '{"info": {"title": "Example API"}}'

$openApi = $openApi->info(null);
echo $openApi->toJson(); // '{}'
```

For variadic setter methods, if you call the method and don't supply any
parameters, then this will have the effect of unsetting the property:

```php
$path = PathItem::create()
    ->route('/users');

$openApi = OpenApi::create()
    ->paths($path);
echo $openApi->toJson(); // '{"paths": {"/users": []}}'

$openApi = $openApi->paths();
echo $openApi->toJson(); // '{}'
```

### Retrieving properties

You can easily retrieve a property using a magic getter. These have been
implemented for all properties for every object. DocBlocks have been provided
to give better auto-completion in IDEs:

```php
$info = Info::create()->title('Example API');

echo $info->title; // 'Example API'
```

### Object ID

Every object has an optional `$objectId` property which is a `string` and can 
either be set in the class constructor or the preferred `create()` method. This 
property is used when a parent object needs to use a name for the children.

An example of this in use is when a schema object is composed of other schema
properties:

```php
$schema = Schema::create()
    ->type(Schema::TYPE_OBJECT)
    ->properties(
        Schema::create('username')->type(Schema::TYPE_STRING),
        Schema::create('age')->type(Schema::TYPE_INTEGER)
    );
    
echo $schema->toJson();
/* 
{
  "type": "object",
  "properties": {
    "username": {
      "type": "string"
    },
    "age": {
      "type": "integer"
    }
  }
} 
*/
``` 

If an object contains any helper creation methods, then these methods also allow
you to specify the `$objectId` property as a parameter. The code sample below is
functionally identical to the one above:

```php
$schema = Schema::object()
    ->properties(
        Schema::string('username'),
        Schema::integer('age')
    );
    
echo $schema->toJson();
/* 
{
  "type": "object",
  "properties": {
    "username": {
      "type": "string"
    },
    "age": {
      "type": "integer"
    }
  }
} 
*/
``` 

### $ref

The use of `$ref` has been applied to every single object to use as you wish.
You may substitute any object for a `$ref` by invoking the `ref()` static method 
on the object class: 

```php
$schema = AllOf::create()
    ->schemas(
        Schema::ref('#/components/schemas/ExampleSchema')
    );
    
echo $schema->toJson();
/*
{
  "allOf": [
    ["$ref": "#/components/schemas/ExampleSchema"]
  ]
}
*/
```

### Specification extensions

You can add [specification extensions](https://github.com/OAI/OpenAPI-Specification/blob/master/versions/3.0.2.md#specificationExtensions)
to all objects:

```php
$schema = Schema::create()
    ->x('foo', 'bar')
    ->x('items', Schema::array()->items(Schema::string()));
    
echo $schema->toJson();
/*
{
  "x-foo": "bar",
  "x-items": {
    "type": "array",
    "items": {
      "type": "string"
    }
  }
}
*/

echo $schema->{'x-foo'}; // 'bar'
```

You can also unset specification extensions by invoking the `x()` method and
only providing the key:

```php
$schema = Schema::create()
    ->x('foo', 'bar')
    ->x('items', Schema::array()->items(Schema::string()));

$schema = $schema->x('foo');
    
echo $schema->toJson();
/*
{
  "x-items": {
    "type": "array",
    "items": {
      "type": "string"
    }
  }
}
*/
```

To retrieve an array of all the specification extensions you can call the `$x`
property:

```php
$schema = Schema::create()
    ->x('foo', 'bar')
    ->x('items', Schema::array()->items(Schema::string()));

echo json_encode($schema->x);
/*
{
  "x-foo": "bar",
  "x-items": {
    "type": "array",
    "items": {
      "type": "string"
    }
  }
}
*/
```

### Validation

In order to perform schema validation you must first install the
`justinrainbow/json-schema` package:

```bash
composer require justinrainbow/json-schema:^5.2
```

Once installed, you may now make use of the `validate()` method on the `OpenApi`
object:

```php
$openApi = OpenApi::create();
$openApi->validate();
```

_If you haven't installed the `justinrainbow/json-schema` package and attempt to 
use the `validate()` method, then a `RuntimeException` will be thrown._

If validation fails for the schema, then a 
`GoldSpecDigital\ObjectOrientedOAS\Exceptions\ValidationException` will be 
thrown. You can use the `getErrors()` method on this exception to retrieve all
of the validation errors.

## Running the tests

To run the test suite you can use the following commands:

```bash
# To run both style and unit tests.
composer test

# To run only style tests.
composer test:style

# To run only unit tests.
composer test:unit
```

If you receive any errors from the style tests, you can automatically fix most,
if not all of the issues with the following command:

```bash
composer fix:style
```

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of 
conduct, and the process for submitting pull requests to us.

## Versioning

We use [SemVer](http://semver.org/) for versioning. For the versions available, 
see the [tags on this repository](https://github.com/goldspecdigital/oooas/tags). 

## Authors

* [GoldSpec Digital](https://github.com/goldspecdigital)

See also the list of [contributors](https://github.com/goldspecdigital/oooas/contributors) 
who participated in this project.

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) 
file for details.
