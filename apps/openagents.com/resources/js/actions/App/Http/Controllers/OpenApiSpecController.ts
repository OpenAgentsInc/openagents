import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../wayfinder'
/**
* @see \App\Http\Controllers\OpenApiSpecController::show
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
export const show = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(options),
    method: 'get',
})

show.definition = {
    methods: ["get","head"],
    url: '/openapi.json',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\OpenApiSpecController::show
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
show.url = (options?: RouteQueryOptions) => {
    return show.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\OpenApiSpecController::show
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
show.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\OpenApiSpecController::show
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
show.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: show.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\OpenApiSpecController::show
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
const showForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\OpenApiSpecController::show
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
showForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\OpenApiSpecController::show
* @see app/Http/Controllers/OpenApiSpecController.php:10
* @route '/openapi.json'
*/
showForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

show.form = showForm

const OpenApiSpecController = { show }

export default OpenApiSpecController