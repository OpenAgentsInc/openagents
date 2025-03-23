import { observer } from "mobx-react-lite"
import { FC } from "react"
import { Alert, Clipboard, ViewStyle } from "react-native"
import { Button, Screen, Text } from "@/components"
import { useHeader } from "@/hooks/useHeader"
import { useStores } from "@/models"
import { goBack } from "@/navigators"
import { WalletStackParamList } from "@/navigators/WalletNavigator"
import * as alert from "@/utils/alert"
import { useNavigation } from "@react-navigation/native"
import { NativeStackNavigationProp, NativeStackScreenProps } from "@react-navigation/native-stack"

type BackupScreenNavigationProp = NativeStackNavigationProp<WalletStackParamList, "BackupWallet">

interface BackupWalletScreenProps
  extends NativeStackScreenProps<WalletStackParamList, "BackupWallet"> { }

export const BackupWalletScreen: FC<BackupWalletScreenProps> = observer(
  function BackupWalletScreen() {
    const { walletStore } = useStores()
    const navigation = useNavigation<BackupScreenNavigationProp>()

    useHeader({
      title: "Backup Wallet",
      leftIcon: "back",
      onLeftPress: goBack,
      rightIcon: "refresh",
      onRightPress: () => navigation.navigate("RestoreWallet"),
    })

    return (
      <Screen
        style={$root}
        preset="scroll"
        contentContainerStyle={{
          flex: 1,
          alignItems: "center",
          paddingHorizontal: 25,
          paddingVertical: 50,
        }}
      >
        <Text text={walletStore.mnemonic ?? "-"} />
        <Button
          text="Copy to clipboard"
          onPress={() => {
            Clipboard.setString(walletStore.mnemonic ?? "-")
            alert.warn({
              title: "Careful now!",
              message:
                "Paste the recovery phrase into your password manager. Then come back to this app and press to empty the clipboard.",
              onOk: () => Clipboard.setString(""),
              okText: "Empty Clipboard",
              err: null,
            })
          }}
          style={{ marginVertical: 50, width: 300 }}
        />
      </Screen>
    )
  },
)

const $root: ViewStyle = {
  flex: 1,
}
