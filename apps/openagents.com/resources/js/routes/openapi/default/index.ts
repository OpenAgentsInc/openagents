import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../wayfinder'
/**
* @see \App\Http\Controllers\OpenApiSpecController::specification
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
export const specification = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: specification.url(options),
    method: 'get',
})

specification.definition = {
    methods: ["get","head"],
    url: '/openapi.json',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\OpenApiSpecController::specification
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
specification.url = (options?: RouteQueryOptions) => {
    return specification.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\OpenApiSpecController::specification
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
specification.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: specification.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\OpenApiSpecController::specification
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
specification.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: specification.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\OpenApiSpecController::specification
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
const specificationForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: specification.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\OpenApiSpecController::specification
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
specificationForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: specification.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\OpenApiSpecController::specification
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
specificationForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: specification.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

specification.form = specificationForm

const defaultMethod = {
    specification: Object.assign(specification, specification),
}

export default defaultMethod