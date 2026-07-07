import type { Meta, StoryObj } from "@storybook/react-native"
import { Image, ImageBackground, View } from "react-native"

import { BackgroundGradient } from "../../components/background-gradient"
import { khalaMobileTheme } from "../../theme/tokens"
import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Text } from "./Text"

const hero = require("../../../assets/images/home-hero.jpg")

const meta = {
  title: "Ignite/Components/Legacy Media And Layout",
  component: View,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

const BulletItem = ({ text }: { text: string }) => (
  <View style={{ alignItems: "flex-start", flexDirection: "row", gap: 10 }}>
    <Text text="•" style={{ color: khalaMobileTheme.accent }} />
    <Text text={text} style={{ flex: 1 }} />
  </View>
)

const FormRow = ({
  children,
  preset,
  style,
}: {
  children: React.ReactNode
  preset: "top" | "middle" | "bottom" | "soloRound" | "soloStraight" | "clear"
  style?: object
}) => {
  const roundedTop = preset === "top" || preset === "soloRound"
  const roundedBottom = preset === "bottom" || preset === "soloRound"
  const clear = preset === "clear"
  return (
    <View
      style={[
        {
          backgroundColor: clear ? "transparent" : "#08111f",
          borderColor: clear ? "transparent" : "#24344a",
          borderTopLeftRadius: roundedTop ? 10 : 0,
          borderTopRightRadius: roundedTop ? 10 : 0,
          borderBottomLeftRadius: roundedBottom ? 10 : 0,
          borderBottomRightRadius: roundedBottom ? 10 : 0,
          borderWidth: 1,
          padding: 14,
        },
        style,
      ]}
    >
      {children}
    </View>
  )
}

export const AutoImageStylePresets: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="With require()" usage="Image sizing variants matching the old AutoImage story.">
        <Image source={hero} style={{ height: 120, width: 120 }} />
        <Image source={hero} style={{ height: 100, width: 150 }} />
        <Image source={hero} style={{ height: 150, width: 150 }} />
        <Image source={hero} style={{ height: 150, width: 110 }} />
        <Image source={hero} resizeMode="contain" style={{ height: 150, width: 150 }} />
      </IgniteUseCase>
      <IgniteUseCase text="With URL" usage="Remote URL shape represented with a stable local asset for offline Storybook.">
        <Image source={hero} style={{ height: 96, width: 96 }} />
        <Image source={hero} resizeMode="cover" style={{ height: 96, width: 150 }} />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const BulletItemStyling: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase noPad text="default" usage="The default usage.">
        <BulletItem text="The default Bullet Item text" />
      </IgniteUseCase>
      <IgniteUseCase noPad text="with view wrapper" usage="The old story wrapped the bullet item in a dark view.">
        <View style={{ backgroundColor: "#02060d", borderColor: "#4fd0ff", borderWidth: 5, padding: 12 }}>
          <BulletItem text="The default Bullet Item text" />
        </View>
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const FormRowAssembled: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="Fully Assembled" usage="FormRow parts designed to fit together.">
        <FormRow preset="top"><Text preset="formLabel">Hello! I am at the top</Text></FormRow>
        <FormRow preset="middle"><Text>Lorem ipsum dolor sit amet, consectetur adipisicing elit. Commodi officia quo rerum impedit asperiores hic.</Text></FormRow>
        <FormRow preset="middle"><Text preset="formHelper">...one more thing</Text></FormRow>
        <FormRow preset="bottom"><Text>Footers!</Text></FormRow>
      </IgniteUseCase>
      <IgniteUseCase text="Alternatives" usage="Less commonly used presets.">
        <FormRow preset="clear"><Text>My borders are still there, but they are clear.</Text></FormRow>
        <FormRow preset="soloRound"><Text>I'm round</Text></FormRow>
        <FormRow preset="soloStraight" style={{ backgroundColor: "#1f2b10", marginTop: 10 }}><Text>I'm square and have a custom style.</Text></FormRow>
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const FormRowPresets: Story = {
  render: () => (
    <IgniteStory>
      {(["top", "middle", "bottom", "soloRound", "soloStraight", "clear"] as const).map((preset) => (
        <IgniteUseCase key={preset} text={preset} usage={`The ${preset} form row preset.`}>
          <FormRow preset={preset}><Text text={`Preset: ${preset}`} /></FormRow>
        </IgniteUseCase>
      ))}
      <IgniteUseCase text="Style array" usage="Form row with an array of styles.">
        <FormRow preset="soloStraight" style={{ borderColor: "#32cd32", borderWidth: 5 }}>
          <Text>Array style.</Text>
        </FormRow>
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const GradientBackgroundStylePresets: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="default/stretch" usage="Full screen background gradient.">
        <BackgroundGradient
          colors={["#422443", "#281b34", "#123044", "#422443"]}
          cornerRadius={8}
          style={{ height: 220, justifyContent: "center", overflow: "hidden", padding: 20 }}
        >
          <Text preset="subheading" text="GradientBackground" />
        </BackgroundGradient>
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const WallpaperStylePresets: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="default/stretch" usage="Full screen wallpaper image.">
        <ImageBackground source={hero} resizeMode="cover" style={{ height: 300, justifyContent: "flex-end", overflow: "hidden", padding: 18 }}>
          <View style={{ backgroundColor: "rgba(2, 6, 13, 0.78)", padding: 12 }}>
            <Text preset="subheading" text="Wallpaper" />
            <Text text="The old Ignite wallpaper story, backed by Khala's hero asset." />
          </View>
        </ImageBackground>
      </IgniteUseCase>
    </IgniteStory>
  ),
}
