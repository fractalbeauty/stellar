@file:Suppress("ktlint:standard:filename")

package net.trillia.stellar

import androidx.compose.ui.window.Window
import androidx.compose.ui.window.awaitApplication
import kotlinx.coroutines.runBlocking
import uniffi.stellar.Core
import uniffi.stellar.CoreException
import uniffi.stellar.logError

fun main() =
    runBlocking {
        val devicesManager = DevicesManager()
        val schemaManager = SchemaManager()

        val core =
            try {
                Core.spawn(profile = "default", devicesChangeHandler = devicesManager, schemaChangeHandler = schemaManager)
            } catch (e: CoreException) {
                logError("Failed to spawn core: $e")
                return@runBlocking
            }

        awaitApplication {
            Window(
                onCloseRequest = ::exitApplication,
                title = "Stellar",
            ) {
                App(
                    core = core,
                    devicesManager = devicesManager,
                    schemaManager = schemaManager,
                )
            }
        }
    }
