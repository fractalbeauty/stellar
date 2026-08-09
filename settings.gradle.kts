rootProject.name = "Stellar"
enableFeaturePreview("TYPESAFE_PROJECT_ACCESSORS")

pluginManagement {
    // Clone and build a fork of Gobley, since we need some fixes that aren't upstreamed yet.
    // We should get rid of this eventually...
    val gobleyForkDir = File(rootDir, ".gradle/gobley-fork")
    val gobleyForkRepo = "https://github.com/fractalbeauty/gobley.git"
    val gobleyForkBranch = "stellar"

    fun git(
        vararg args: String,
        workingDir: File,
    ) {
        val result =
            providers.exec {
                commandLine(listOf("git") + args)
                this.workingDir = workingDir
            }
        result.result.get().assertNormalExitValue()
    }

    if (!gobleyForkDir.exists()) {
        git(
            "clone",
            "--branch",
            gobleyForkBranch,
            "--single-branch",
            gobleyForkRepo,
            gobleyForkDir.absolutePath,
            workingDir = rootDir,
        )
    } else {
        git("fetch", "origin", gobleyForkBranch, workingDir = gobleyForkDir)
        git("reset", "--hard", "origin/$gobleyForkBranch", workingDir = gobleyForkDir)
    }

    includeBuild(File(gobleyForkDir, "build-logic"))

    repositories {
        google {
            mavenContent {
                includeGroupAndSubgroups("androidx")
                includeGroupAndSubgroups("com.android")
                includeGroupAndSubgroups("com.google")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google {
            mavenContent {
                includeGroupAndSubgroups("androidx")
                includeGroupAndSubgroups("com.android")
                includeGroupAndSubgroups("com.google")
            }
        }
        mavenCentral()
    }
}

plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

include(":androidApp")
include(":desktopApp")
include(":shared")
