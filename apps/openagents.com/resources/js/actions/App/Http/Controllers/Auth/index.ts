import EmailCodeAuthController from './EmailCodeAuthController'
import LocalTestLoginController from './LocalTestLoginController'

const Auth = {
    EmailCodeAuthController: Object.assign(EmailCodeAuthController, EmailCodeAuthController),
    LocalTestLoginController: Object.assign(LocalTestLoginController, LocalTestLoginController),
}

export default Auth