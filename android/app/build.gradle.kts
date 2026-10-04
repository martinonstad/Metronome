plugins {
  alias(libs.plugins.android.application)
  alias(libs.plugins.compose.compiler)
}

android {
    namespace = "no.onstad.metronom"
    compileSdk = 36
    defaultConfig {
        applicationId = "no.onstad.metronom"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        // The Rust library is only built for arm64; this also keeps JNA from bundling
        // native helpers for ABIs we do not ship.
        ndk { abiFilters += "arm64-v8a" }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
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
