import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../wayfinder'
/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
export const email = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: email.url(options),
    method: 'post',
})

email.definition = {
    methods: ["post"],
    url: '/login/email',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
email.url = (options?: RouteQueryOptions) => {
    return email.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
email.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: email.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
const emailForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: email.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:36
* @route '/login/email'
*/
emailForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: email.url(options),
    method: 'post',
})

email.form = emailForm

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
export const verify = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: verify.url(options),
    method: 'post',
})

verify.definition = {
    methods: ["post"],
    url: '/login/verify',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
verify.url = (options?: RouteQueryOptions) => {
    return verify.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
verify.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: verify.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
const verifyForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: verify.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:69
* @route '/login/verify'
*/
verifyForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: verify.url(options),
    method: 'post',
})

verify.form = verifyForm

const login = {
    email: Object.assign(email, email),
    verify: Object.assign(verify, verify),
}

export default login