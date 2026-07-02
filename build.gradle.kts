plugins {
    // this is necessary to avoid the plugins to be loaded multiple times
    // in each subproject's classloader
    alias(libs.plugins.androidApplication) apply false
    alias(libs.plugins.androidLibrary) apply false
    alias(libs.plugins.kotlinAndroid) apply false
    alias(libs.plugins.composeMultiplatform) apply false
    alias(libs.plugins.composeCompiler) apply false
    alias(libs.plugins.kotlinJvm) apply false
    alias(libs.plugins.kotlinMultiplatform) apply false

    alias(libs.plugins.kotlinAtomicfu) apply false
    alias(libs.plugins.gobleyCargo) apply false
    alias(libs.plugins.gobleyUniffi) apply false
    alias(libs.plugins.ktlint) apply false
}

subprojects {
    apply(plugin = "org.jlleitschuh.gradle.ktlint")

    configure<org.jlleitschuh.gradle.ktlint.KtlintExtension> {
        // Compose registers generated sources (resource accessors, etc.) under build/
        // as Kotlin source sets. Only lint our own source, not generated code.
        filter {
            exclude { element -> element.file.path.contains("/build/") }
        }
    }
}
