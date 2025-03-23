import React, { memo, ReactElement, useMemo } from "react"
import { StyleProp, StyleSheet, View, ViewStyle } from "react-native"
import { Text } from "@/components/Text"
import { typography } from "@/theme"
import { colors } from "@/theme/colorsDark"
import { EDenomination } from "./MoneySmall"

export enum EUnit {
  BTC = "BTC",
  fiat = "fiat",
}

type TSize = "display" | "title" | "bodyMSB" | "bodySSB" | "captionB" | "caption13Up"

type MoneyProps = {
  sats: number
  unitType?: "primary" | "secondary" // force primary or secondary unit. Can be overwritten by unit prop
  unit?: EUnit // force value formatting
  decimalLength?: "long" | "short" // whether to show 5 or 8 decimals for BTC
  symbol?: boolean // show symbol icon
  symbolColor?: string // keyof IThemeColors;
  color?: string // keyof IThemeColors;
  enableHide?: boolean // if true and settings.hideBalance === true it will replace number with dots
  sign?: string
  shouldRoundUp?: boolean
  style?: StyleProp<ViewStyle>
  testID?: string
  size?: TSize
}

const Money = (props: MoneyProps): ReactElement => {
  // const primaryUnit = useAppSelector(unitSelector);
  // const nextUnit = useAppSelector(nextUnitSelector);
  // const denomination = useAppSelector(denominationSelector);
  // const hideBalance = useAppSelector(hideBalanceSelector);

  const primaryUnit = EUnit.BTC
  const nextUnit = EUnit.fiat
  const denomination = EDenomination.modern as EDenomination
  const hideBalance = false

  const sats = Math.abs(props.sats)
  const decimalLength = props.decimalLength ?? "long"
  const size = props.size ?? "display"
  const unit = props.unit ?? (props.unitType === "secondary" ? nextUnit : primaryUnit)
  const showSymbol = props.symbol ?? (unit === "fiat" ? true : false)
  const color = props.color
  const symbolColor = props.symbolColor
  const hide = (props.enableHide ?? false) && hideBalance
  const sign = props.sign
  const shouldRoundUp = props.shouldRoundUp ?? false
  const testID = props.testID

  // const dv = useDisplayValues(sats, shouldRoundUp);

  const dv = {
    fiatWhole: "600",
    fiatFormatted: "600",
    bitcoinFormatted: sats,
    fiatSymbol: "$",
  }

  // const [Text, iconMargin] = useMemo(() => {
  //   switch (size) {
  //     case 'captionB':
  //       return [Caption13Up, 3];
  //     case 'caption13Up':
  //       return [CaptionB, 4];
  //     case 'bodyMSB':
  //       return [BodyMSB, 4];
  //     case 'bodySSB':
  //       return [BodySSB, 4];
  //     case 'title':
  //       return [Title, 6];
  //     default:
  //       return [Display, 6];
  //   }
  // }, [size]);

  const symbol = useMemo(() => {
    const style = {
      marginTop: -4,
      marginRight: 5, // iconMargin,
      fontSize: 30,
      lineHeight: 40,
      color: colors.palette.accent100,
      fontFamily: typography.secondary.bold,
    }

    return (
      <Text
        style={style}
        // color={symbolColor ?? color ?? 'secondary'}
        testID="MoneyFiatSymbol"
      >
        {unit === EUnit.BTC ? "₿" : dv.fiatSymbol}
      </Text>
    )
  }, [Text, size, unit, color, symbolColor, dv.fiatSymbol]) // iconMargin

  let text = useMemo(() => {
    switch (unit) {
      case EUnit.fiat: {
        if (dv.fiatWhole.length > 12) {
          // const { newValue, abbreviation } = abbreviateNumber(dv.fiatWhole);
          const newValue = 600
          const abbreviation = "k"
          return `${newValue}${abbreviation}`
        }

        return dv.fiatFormatted
      }
      case EUnit.BTC: {
        if (denomination === EDenomination.classic) {
          if (decimalLength === "long") {
            return Number(dv.bitcoinFormatted).toFixed(8)
          }

          return Number(dv.bitcoinFormatted).toFixed(5)
        }

        return dv.bitcoinFormatted
      }
    }
  }, [dv, unit, denomination, decimalLength])

  if (hide) {
    if (size === "display") {
      text = " • • • • • • • • •"
    } else {
      text = " • • • • •"
    }
  }

  return (
    <View style={[styles.root, props.style]} testID={testID}>
      {sign && (
        <Text
          style={styles.sign}
          // color={color ?? 'secondary'}
          testID="MoneySign"
        >
          {sign}
        </Text>
      )}
      {showSymbol && symbol}
      <Text
        // color={color}
        style={styles.balance}
        testID="MoneyText"
      >
        {text}
      </Text>
    </View>
  )
}

const styles = StyleSheet.create({
  root: {
    flexDirection: "row",
    alignItems: "center",
  },
  sign: {
    marginRight: 3,
  },
  balance: {
    fontSize: 34,
    lineHeight: 40,
    fontFamily: typography.primary.bold,
  },
})

export default memo(Money)
