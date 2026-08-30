import gobley.gradle.GobleyHost
import gobley.gradle.Variant
import gobley.gradle.cargo.dsl.appleMobile
import gobley.gradle.cargo.dsl.jvm
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlinMultiplatform)
    alias(libs.plugins.androidLibrary)
    alias(libs.plugins.composeMultiplatform)
    alias(libs.plugins.composeCompiler)

    alias(libs.plugins.gobleyCargo)
    alias(libs.plugins.gobleyUniffi)
    alias(libs.plugins.kotlinAtomicfu)
}

android {
    namespace = "net.trillia.stellar.shared"
    compileSdk =
        libs.versions.android.compileSdk
            .get()
            .toInt()
    defaultConfig {
        minSdk =
            libs.versions.android.minSdk
                .get()
                .toInt()
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    testOptions {
        unitTests {
            isIncludeAndroidResources = true
        }
    }
}

kotlin {
    listOf(
        iosArm64(),
        iosSimulatorArm64(),
    ).forEach { iosTarget ->
        iosTarget.binaries.framework {
            baseName = "Shared"
            isStatic = true
        }
    }

    jvm()

    androidTarget {
        compilerOptions {
            jvmTarget = JvmTarget.JVM_11
        }
    }

    sourceSets {
        androidMain.dependencies {
            implementation(libs.compose.uiToolingPreview)
        }
        commonMain.dependencies {
            implementation(libs.compose.runtime)
            implementation(libs.compose.foundation)
            implementation(libs.compose.material3)
            implementation(libs.compose.ui)
            implementation(libs.compose.components.resources)
            implementation(libs.compose.uiToolingPreview)
            implementation(libs.androidx.lifecycle.viewmodelCompose)
            implementation(libs.androidx.lifecycle.viewmodelNavigation3)
            implementation(libs.androidx.lifecycle.runtimeCompose)
        }
        commonTest.dependencies {
            implementation(libs.kotlin.test)
        }
    }
}

cargo {
    packageDirectory = rootProject.layout.projectDirectory.dir("crates/stellar")

    // Allow setting `-Pnet.trillia.stellar.rust.variant=release` to build Rust in release mode
    jvmVariant = Variant(findProperty("net.trillia.stellar.rust.variant")?.toString() ?: "debug")

    builds.jvm {
        // Build desktop for the host target only
        embedRustLibrary = rustTarget == GobleyHost.current.rustTarget
    }

    // Don't install rustup targets automatically
    installTargetBeforeBuild = false

    builds.appleMobile {
        variants {
            buildTaskProvider.configure {
                when (rustTarget.cinteropName) {
                    // Set the iOS deployment target to match XCode. Without this we get linking errors.
                    "ios" -> {
                        additionalEnvironment.put("IPHONEOS_DEPLOYMENT_TARGET", "16.0.0")
                    }

                    else -> {}
                }
            }
        }
    }
}

uniffi {
    // Added in our temporary fork of Gobley, see `settings.gradle.kt`.
    bindgenFromPath(rootProject.layout.projectDirectory.dir(".gradle/gobley-fork/crates/gobley-uniffi-bindgen"))

    generateFromLibrary {
        // Build bindings from the host target since we're mostly running the desktop app for now.
        build = GobleyHost.current.rustTarget
        // Added in our temporary fork of Gobley, see `settings.gradle.kt`.
        generateAllNamespaces = true

        customType("Uuid") {
            typeName = "kotlin.uuid.Uuid"
            lift = "kotlin.uuid.Uuid.fromByteArray({})"
            lower = "{}.toByteArray()"
        }
        customType("EntityId") {
            typeName = "net.trillia.stellar.BytesValue"
            lift = "net.trillia.stellar.BytesValue.fromByteArray({})"
            lower = "{}.toByteArray()"
        }
        customType("RelationId") {
            typeName = "net.trillia.stellar.BytesValue"
            lift = "net.trillia.stellar.BytesValue.fromByteArray({})"
            lower = "{}.toByteArray()"
        }
        customType("EntityKind") {
            typeName = "net.trillia.stellar.BytesValue"
            lift = "net.trillia.stellar.BytesValue.fromByteArray({})"
            lower = "{}.toByteArray()"
        }
        customType("RelationKind") {
            typeName = "net.trillia.stellar.BytesValue"
            lift = "net.trillia.stellar.BytesValue.fromByteArray({})"
            lower = "{}.toByteArray()"
        }
        customType("AttributeKind") {
            typeName = "net.trillia.stellar.BytesValue"
            lift = "net.trillia.stellar.BytesValue.fromByteArray({})"
            lower = "{}.toByteArray()"
        }
    }
}

dependencies {
    debugImplementation(libs.compose.uiTooling)
}
