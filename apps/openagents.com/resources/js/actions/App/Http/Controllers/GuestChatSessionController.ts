import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../wayfinder'
/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
const GuestChatSessionController = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: GuestChatSessionController.url(options),
    method: 'get',
})

GuestChatSessionController.definition = {
    methods: ["get","head"],
    url: '/api/chat/guest-session',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
GuestChatSessionController.url = (options?: RouteQueryOptions) => {
    return GuestChatSessionController.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
GuestChatSessionController.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: GuestChatSessionController.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
GuestChatSessionController.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: GuestChatSessionController.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
const GuestChatSessionControllerForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: GuestChatSessionController.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
GuestChatSessionControllerForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: GuestChatSessionController.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
GuestChatSessionControllerForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: GuestChatSessionController.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

GuestChatSessionController.form = GuestChatSessionControllerForm

export default GuestChatSessionController