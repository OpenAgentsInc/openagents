import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../wayfinder'
/**
* @see \App\Http\Controllers\FeedPageController::index
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
export const index = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

index.definition = {
    methods: ["get","head"],
    url: '/feed',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\FeedPageController::index
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
index.url = (options?: RouteQueryOptions) => {
    return index.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\FeedPageController::index
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
index.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\FeedPageController::index
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
index.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: index.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\FeedPageController::index
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
const indexForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\FeedPageController::index
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
indexForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\FeedPageController::index
* @see app/Http/Controllers/FeedPageController.php:13
* @route '/feed'
*/
indexForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

index.form = indexForm

const FeedPageController = { index }

export default FeedPageController