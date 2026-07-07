import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { BackgroundGradient } from "../../components/background-gradient"
import { KhalaAutoImage } from "../../components/khala-auto-image"
import { KhalaBulletItem } from "../../components/khala-bullet-item"
import { KhalaFormRow } from "../../components/khala-form-row"
import { KhalaWallpaper } from "../../components/khala-wallpaper"
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

export const AutoImageStylePresets: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="With require()" usage="Image sizing variants matching the old AutoImage story.">
        <KhalaAutoImage source={hero} style={{ height: 120, width: 120 }} />
        <KhalaAutoImage source={hero} style={{ height: 100, width: 150 }} />
        <KhalaAutoImage source={hero} style={{ height: 150, width: 150 }} />
        <KhalaAutoImage source={hero} style={{ height: 150, width: 110 }} />
        <KhalaAutoImage source={hero} resizeMode="contain" style={{ height: 150, width: 150 }} />
      </IgniteUseCase>
      <IgniteUseCase text="With URL" usage="Remote URL shape represented with a stable local asset for offline Storybook.">
        <KhalaAutoImage source={hero} style={{ height: 96, width: 96 }} />
        <KhalaAutoImage source={hero} resizeMode="cover" style={{ height: 96, width: 150 }} />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const BulletItemStyling: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase noPad text="default" usage="The default usage.">
        <KhalaBulletItem text="The default Bullet Item text" />
      </IgniteUseCase>
      <IgniteUseCase noPad text="with view wrapper" usage="The old story wrapped the bullet item in a dark view.">
        <View style={{ backgroundColor: "#02060d", borderColor: "#4fd0ff", borderWidth: 5, padding: 12 }}>
          <KhalaBulletItem text="The default Bullet Item text" />
        </View>
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const FormRowAssembled: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="Fully Assembled" usage="FormRow parts designed to fit together.">
        <KhalaFormRow preset="top"><Text preset="formLabel">Hello! I am at the top</Text></KhalaFormRow>
        <KhalaFormRow preset="middle"><Text>Lorem ipsum dolor sit amet, consectetur adipisicing elit. Commodi officia quo rerum impedit asperiores hic.</Text></KhalaFormRow>
        <KhalaFormRow preset="middle"><Text preset="formHelper">...one more thing</Text></KhalaFormRow>
        <KhalaFormRow preset="bottom"><Text>Footers!</Text></KhalaFormRow>
      </IgniteUseCase>
      <IgniteUseCase text="Alternatives" usage="Less commonly used presets.">
        <KhalaFormRow preset="clear"><Text>My borders are still there, but they are clear.</Text></KhalaFormRow>
        <KhalaFormRow preset="soloRound"><Text>I'm round</Text></KhalaFormRow>
        <KhalaFormRow preset="soloStraight" style={{ backgroundColor: "#1f2b10", marginTop: 10 }}><Text>I'm square and have a custom style.</Text></KhalaFormRow>
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const FormRowPresets: Story = {
  render: () => (
    <IgniteStory>
      {(["top", "middle", "bottom", "soloRound", "soloStraight", "clear"] as const).map((preset) => (
        <IgniteUseCase key={preset} text={preset} usage={`The ${preset} form row preset.`}>
          <KhalaFormRow preset={preset}><Text text={`Preset: ${preset}`} /></KhalaFormRow>
        </IgniteUseCase>
      ))}
      <IgniteUseCase text="Style array" usage="Form row with an array of styles.">
        <KhalaFormRow preset="soloStraight" style={{ borderColor: "#32cd32", borderWidth: 5 }}>
          <Text>Array style.</Text>
        </KhalaFormRow>
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
        <KhalaWallpaper style={{ height: 300, justifyContent: "flex-end", padding: 18 }}>
          <View style={{ padding: 12 }}>
            <Text preset="subheading" text="Wallpaper" />
            <Text text="The old Ignite wallpaper story, backed by Khala's hero asset." />
          </View>
        </KhalaWallpaper>
      </IgniteUseCase>
    </IgniteStory>
  ),
}
