plugins {
  alias(libs.plugins.android.application)
  alias(libs.plugins.compose.compiler)
}

// The one place the version is set. versionCode is derived from it (0.9.0 -> 900, 1.0.0 -> 10000),
// so it always grows with the version; scripts/build-release.sh reads this line too.
val appVersionName = "0.9.0"
val appVersionCode =
    appVersionName.substringBefore('-').split('.').let { (major, minor, patch) ->
        major.toInt() * 10_000 + minor.toInt() * 100 + patch.toInt()
    }

android {
    // The Kotlin package; the id the phone knows the app by is applicationId below.
    namespace = "no.onstad.metronom"
    compileSdk = 36
    defaultConfig {
        // Permanent once a release is installed anywhere: a different id is a different app.
        applicationId = "io.github.martinonstad.metronom"
        minSdk = 26
        targetSdk = 36
        versionCode = appVersionCode
        versionName = appVersionName
        manifestPlaceholders["appLabel"] = "@string/app_name"
        // The Rust library is only built for arm64; this also keeps JNA from bundling
        // native helpers for ABIs we do not ship.
        ndk { abiFilters += "arm64-v8a" }
    }

    // Release signing comes from the environment, so no key or password is ever in the repository
    // (see docs/development.md). Without it `assembleRelease` makes an unsigned APK, which is
    // enough to check the size but cannot be installed.
    fun signing(name: String) = providers.environmentVariable(name).orNull
    val keystore = signing("METRONOM_KEYSTORE")
    if (keystore != null) {
        signingConfigs {
            create("release") {
                storeFile = file(keystore)
                storePassword = signing("METRONOM_KEYSTORE_PASSWORD")
                keyAlias = signing("METRONOM_KEY_ALIAS")
                keyPassword = signing("METRONOM_KEY_PASSWORD") ?: signing("METRONOM_KEYSTORE_PASSWORD")
            }
        }
    }

    buildTypes {
        // The debug build has its own id and label, so a debug and a release build can be
        // installed side by side (and a release never has to replace, and wipe, the debug one).
        debug {
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
            manifestPlaceholders["appLabel"] = "Metronom (debug)"
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
      compose = true
      aidl = false
      buildConfig = false
      shaders = false
    }

    packaging {
      resources {
        excludes += "/META-INF/{AL2.0,LGPL2.1}"
      }
    }
}

// Build the Rust core for Android and regenerate the Kotlin bindings before every build, so
// `./gradlew installDebug` is the only command needed.
val repoRoot = rootDir.parentFile
val buildRust by tasks.registering(Exec::class) {
    group = "build"
    description = "Builds libmetronom_ffi.so for arm64 and generates the UniFFI Kotlin bindings"
    workingDir = repoRoot
    commandLine("scripts/build-android-libs.sh")
    inputs.files(fileTree(File(repoRoot, "core")) { exclude("target/**") })
    inputs.file(File(repoRoot, "scripts/build-android-libs.sh"))
    outputs.dir("src/main/jniLibs")
    outputs.dir("src/main/java/uniffi")
}
tasks.named("preBuild") { dependsOn(buildRust) }

dependencies {
  val composeBom = platform(libs.androidx.compose.bom)
  implementation(composeBom)

  implementation(libs.androidx.core.ktx)
  implementation(libs.androidx.lifecycle.runtime.ktx)
  implementation(libs.androidx.lifecycle.runtime.compose)
  implementation(libs.androidx.activity.compose)

  implementation(libs.androidx.compose.ui)
  implementation(libs.androidx.compose.ui.tooling.preview)
  implementation(libs.androidx.compose.material3)
  debugImplementation(libs.androidx.compose.ui.tooling)

  // Required at runtime by the UniFFI-generated Kotlin bindings; the aar carries the native part.
  implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")

  testImplementation(libs.junit)
  testImplementation(libs.kotlinx.coroutines.test)
}
