plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val coderAbi = providers.gradleProperty("coderAbi").orElse("arm64-v8a").get()
require(coderAbi in setOf("arm64-v8a", "x86_64")) { "Unsupported Coder Android ABI" }
val nativeDirectory = providers.gradleProperty("coderNativeDir")
    .orElse(rootProject.file("../../../../target/coder-android/jniLibs").absolutePath)
// Push is opt-in: Firebase Messaging compiles in only with google-services.json,
// and the app registers only when the three push settings are also set.
val firebase = file("google-services.json").exists()
if (firebase) apply(plugin = "com.google.gms.google-services")
fun pushSetting(name: String) = providers.gradleProperty(name).orElse("").get()
    .also { require(!it.contains('"') && !it.contains('\\')) { "Invalid $name" } }

android {
    namespace = "com.openagents.coder"
    compileSdk = 35
    ndkVersion = "27.1.12297006"

    defaultConfig {
        applicationId = "com.openagents.coder"
        minSdk = 26
        targetSdk = 35
        versionCode = 5
        versionName = "0.5.0"
        ndk { abiFilters += coderAbi }
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        buildConfigField("String", "CODER_PUSH_RELAY_URL", "\"${pushSetting("coderPushRelayUrl")}\"")
        buildConfigField("String", "CODER_PUSH_GATEWAY_URL", "\"${pushSetting("coderPushGatewayUrl")}\"")
        buildConfigField("String", "CODER_PUSH_APP_PROFILE", "\"${pushSetting("coderPushAppProfile")}\"")
        manifestPlaceholders["coderPushEnabled"] = firebase.toString()
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { buildConfig = true }
    sourceSets.getByName("main").jniLibs.srcDir(nativeDirectory)
    sourceSets.getByName("main").java.srcDir(if (firebase) "src/push/java" else "src/nopush/java")
    packaging { jniLibs.useLegacyPackaging = false }
    buildTypes {
        debug { isJniDebuggable = true }
        release { isMinifyEnabled = false }
    }
    testOptions { animationsDisabled = true }
}

// Paper Mono's static faces under Android resource names. The repository
// keeps one copy in crates/paper-mono/fonts; this task copies it into a
// generated resource directory, and res/font/paper_mono.xml names the faces.
abstract class PaperMonoFonts : DefaultTask() {
    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val source: DirectoryProperty

    @get:OutputDirectory
    abstract val output: DirectoryProperty

    @TaskAction
    fun copy() {
        val font = output.get().dir("font").asFile
        font.deleteRecursively()
        font.mkdirs()
        mapOf(
            "PaperMono-Regular.ttf" to "paper_mono_regular.ttf",
            "PaperMono-Medium.ttf" to "paper_mono_medium.ttf",
            "PaperMono-SemiBold.ttf" to "paper_mono_semibold.ttf",
            "PaperMono-Bold.ttf" to "paper_mono_bold.ttf",
        ).forEach { (from, to) -> source.get().file(from).asFile.copyTo(font.resolve(to)) }
    }
}

val paperMonoFonts = tasks.register<PaperMonoFonts>("paperMonoFonts") {
    source.set(rootProject.file("../../../crates/paper-mono/fonts"))
}

androidComponents {
    onVariants { variant ->
        variant.sources.res?.addGeneratedSourceDirectory(paperMonoFonts, PaperMonoFonts::output)
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.camera:camera-core:1.4.2")
    implementation("androidx.camera:camera-camera2:1.4.2")
    implementation("androidx.camera:camera-lifecycle:1.4.2")
    implementation("androidx.camera:camera-view:1.4.2")
    implementation("com.google.zxing:core:3.5.3")
    implementation("io.noties.markwon:core:4.6.2")
    if (firebase) implementation("com.google.firebase:firebase-messaging:24.1.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:core:1.6.1")
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
}
