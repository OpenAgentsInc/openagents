import * as AuthSession from "expo-auth-session";
import * as WebBrowser from "expo-web-browser";
import { Schema as S } from "effect";
import { useEffect, useRef, useState } from "react";
import { ActivityIndicator, View, type TextStyle, type ViewStyle } from "react-native";

import type { NativeSessionCredential } from "../auth/native-session-vault";
import { Button } from "../ui/button";
import { Text } from "../ui/text";
import { colors, spacing } from "../ui/theme";

WebBrowser.maybeCompleteAuthSession();

const CLIENT_ID = "openagents-khala-mobile";
const REDIRECT_URI = "openagents://auth";
const OPENAGENTS_API = "https://openagents.com";
const discovery = {
  authorizationEndpoint: "https://auth.openagents.com/authorize",
  tokenEndpoint: "https://auth.openagents.com/token",
};

const MobileSession = S.Struct({
  authenticated: S.Literal(true),
  user: S.Struct({ userId: S.String }),
});

export const exchangeMobileCredential = async (input: {
  readonly code: string;
  readonly codeVerifier: string;
  readonly exchange?: typeof AuthSession.exchangeCodeAsync;
  readonly fetch?: typeof globalThis.fetch;
}): Promise<NativeSessionCredential> => {
  const token = await (input.exchange ?? AuthSession.exchangeCodeAsync)(
    {
      clientId: CLIENT_ID,
      code: input.code,
      redirectUri: REDIRECT_URI,
      extraParams: { code_verifier: input.codeVerifier },
    },
    discovery,
  );
  if (token.refreshToken === undefined || token.refreshToken.trim() === "") {
    throw new Error("The sign-in response did not include a refresh session.");
  }
  const sessionResponse = await (input.fetch ?? globalThis.fetch)(
    `${OPENAGENTS_API}/api/mobile/auth/session`,
    { headers: { authorization: `Bearer ${token.accessToken}` } },
  );
  if (!sessionResponse.ok) throw new Error("OpenAgents could not verify this sign-in.");
  const session = S.decodeUnknownSync(MobileSession)(await sessionResponse.json());
  return {
    ownerUserId: session.user.userId,
    accessToken: token.accessToken,
    refreshToken: token.refreshToken,
  };
};

export const ControllerSignInScreen = ({
  onCredential,
}: {
  readonly onCredential: (credential: NativeSessionCredential) => Promise<void>;
}) => {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const exchangedCode = useRef<string | null>(null);
  const [request, response, promptAsync] = AuthSession.useAuthRequest(
    {
      clientId: CLIENT_ID,
      redirectUri: REDIRECT_URI,
      responseType: AuthSession.ResponseType.Code,
      usePKCE: true,
      codeChallengeMethod: AuthSession.CodeChallengeMethod.S256,
      extraParams: { provider: "github" },
    },
    discovery,
  );

  useEffect(() => {
    if (response?.type !== "success") return;
    const code = response.params.code;
    const verifier = request?.codeVerifier;
    if (typeof code !== "string" || verifier === undefined || exchangedCode.current === code)
      return;
    exchangedCode.current = code;
    setBusy(true);
    setError(null);
    void exchangeMobileCredential({ code, codeVerifier: verifier })
      .then(onCredential)
      .catch((cause: unknown) => {
        exchangedCode.current = null;
        setError(cause instanceof Error ? cause.message : "Sign in failed.");
      })
      .finally(() => setBusy(false));
  }, [onCredential, request?.codeVerifier, response]);

  return (
    <View style={$screen} accessibilityRole="summary">
      <View style={$mark} accessibilityElementsHidden>
        <Text preset="heading" color={colors.accentInk}>
          OA
        </Text>
      </View>
      <Text preset="display" style={$title}>
        Your work, within reach.
      </Text>
      <Text preset="body" color={colors.textDim} style={$body}>
        Sign in to see your live attention inbox, answer requests, and steer active work from this
        phone.
      </Text>
      {error === null ? null : (
        <Text preset="caption" color={colors.fault} style={$error}>
          {error}
        </Text>
      )}
      <Button
        label={busy ? "Signing in…" : "Sign in with GitHub"}
        disabled={request === null || busy}
        onPress={() => {
          setError(null);
          void promptAsync();
        }}
        fullWidth
      >
        {busy ? <ActivityIndicator color={colors.palette.void} /> : undefined}
      </Button>
      <Text preset="caption" color={colors.textFaint} style={$privacy}>
        Credentials stay in the device keychain. Pro receives a short-lived controller session only.
      </Text>
    </View>
  );
};

const $screen: ViewStyle = {
  flex: 1,
  justifyContent: "center",
  padding: spacing.large,
  backgroundColor: colors.background,
};
const $mark: ViewStyle = {
  width: 52,
  height: 52,
  borderRadius: 12,
  borderWidth: 1,
  borderColor: colors.borderEnergized,
  backgroundColor: colors.accentGlow,
  alignItems: "center",
  justifyContent: "center",
  marginBottom: spacing.large,
};
const $title: TextStyle = { marginBottom: spacing.small };
const $body: TextStyle = { marginBottom: spacing.large, maxWidth: 520 };
const $error: TextStyle = { marginBottom: spacing.medium };
const $privacy: TextStyle = { marginTop: spacing.medium, textAlign: "center" };
