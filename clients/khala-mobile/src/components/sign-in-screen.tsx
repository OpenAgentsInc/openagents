import { View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { tx } from "../i18n/copy"
import { KhalaButton } from "./khala-button"
import { KhalaScreen } from "./khala-screen"
import { KhalaText } from "./khala-text"

export const SignInScreen = () => {
  const {
    githubSignInReady,
    signInErrorMessage,
    signInWithGitHub,
    status,
  } = useKhalaAuth()
  const signingIn = status === "signing_in"

  return (
    <KhalaScreen contentClassName="justify-center px-7">
      <View>
        <KhalaText className="mb-3 text-center text-3xl" variant="heading">
          {tx("app.title")}
        </KhalaText>
        <KhalaText className="mb-8 text-center leading-6 text-textBody" variant="body">
          {tx("signIn.github.subtitle")}
        </KhalaText>

        <KhalaButton
          disabled={!githubSignInReady}
          loading={signingIn}
          onPress={signInWithGitHub}
          text={tx("signIn.github.primary")}
          variant="primary"
        />

        {signInErrorMessage === null ? null : (
          <KhalaText className="mt-4 text-center" variant="danger">
            {signInErrorMessage}
          </KhalaText>
        )}

        <KhalaText className="mt-6 text-center leading-5" variant="faint">
          {tx("signIn.github.note")}
        </KhalaText>
      </View>
    </KhalaScreen>
  )
}
