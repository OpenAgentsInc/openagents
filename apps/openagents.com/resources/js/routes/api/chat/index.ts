import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../wayfinder'
/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
export const guestSession = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: guestSession.url(options),
    method: 'get',
})

guestSession.definition = {
    methods: ["get","head"],
    url: '/api/chat/guest-session',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
guestSession.url = (options?: RouteQueryOptions) => {
    return guestSession.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
guestSession.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: guestSession.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
guestSession.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: guestSession.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
const guestSessionForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: guestSession.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
guestSessionForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: guestSession.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\GuestChatSessionController::__invoke
* @see app/Http/Controllers/GuestChatSessionController.php:12
* @route '/api/chat/guest-session'
*/
guestSessionForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: guestSession.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

guestSession.form = guestSessionForm
