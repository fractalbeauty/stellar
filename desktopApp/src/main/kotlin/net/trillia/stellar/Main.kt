package net.trillia.stellar

import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.awaitApplication
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.runBlocking
import net.trillia.stellar.desktop.DesktopApp
import net.trillia.stellar.mobile.MobileApp
import uniffi.stellar.Core
import uniffi.stellar.CoreException

fun main(args: Array<String>) =
    runBlocking {
        val isMobile = "--mobile" in args

        val devicesManager = DevicesManager()
        val schemaManager = SchemaManager()

        val profile = resolveProfile(args)

        val core =
            try {
                Core.spawn(profile = profile, devicesChangeHandler = devicesManager, schemaChangeHandler = schemaManager)
            } catch (e: CoreException) {
                logCoreError(e, "Failed to spawn core")
                return@runBlocking
            }

        val title = if (profile == "default") "Stellar" else "Stellar (profile: $profile)"

        awaitApplication {
            val windowState =
                if (isMobile) rememberWindowState(size = DpSize(320.dp, 568.dp)) else rememberWindowState()

            Window(
                onCloseRequest = ::exitApplication,
                title = title,
                state = windowState,
            ) {
                if (isMobile) {
                    MobileApp(core, devicesManager, schemaManager)
                } else {
                    DesktopApp(core, devicesManager, schemaManager)
                }
            }
        }
    }

/**
 * Resolves the profile to use from `--profile <name>`/`-p <name>` or `--anonymous`/`-a`.
 */
private fun resolveProfile(args: Array<String>): String {
    val named = parseProfileArg(args)
    if (named != null) return named
    if (args.any { it == "--anonymous" || it == "-a" }) {
        val id = (1..7).map { ('a'..'z').random() }.joinToString("")
        return "anonymous-$id"
    }
    return "default"
}

private fun parseProfileArg(args: Array<String>): String? {
    val index = args.indexOfFirst { it == "--profile" || it == "-p" }
    if (index == -1 || index + 1 >= args.size) return null
    return args[index + 1]
}
