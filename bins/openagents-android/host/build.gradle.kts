plugins {
    id("com.android.application") version "8.13.0" apply false
    id("org.jetbrains.kotlin.android") version "2.0.21" apply false
}

// Keep generated artifacts outside the source checkout, including IDE builds.
val output = providers.gradleProperty("openagentsOutputDir")
    .orElse(file("../../../../target/openagents-android/gradle").absolutePath)
layout.buildDirectory.set(file(output.get()).resolve("root"))
subprojects {
    layout.buildDirectory.set(file(output.get()).resolve(name))
}
