package net.trillia.stellar

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.input.pointer.PointerKeyboardModifiers
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.platform.WindowInfo
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.v2.runComposeUiTest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import java.io.File
import java.lang.StackWalker

@OptIn(ExperimentalTestApi::class)
fun uiTest(block: ComposeUiTest.() -> Unit) {
    // Get the calling test's name
    val callerFrame = StackWalker.getInstance().walk { frames -> frames.skip(1).findFirst().get() }
    val name = "${callerFrame.className.substringAfterLast('.')}.${callerFrame.methodName}"

    runComposeUiTest {
        // Stash the test name
        currentUiTest.set(UiTestContext(name))

        try {
            block()
        } catch (t: Throwable) {
            saveScreenshot(onRoot().captureToImage(), "$name.failure")
            throw t
        } finally {
            currentUiTest.remove()
        }
    }
}

@OptIn(ExperimentalTestApi::class)
fun ComposeUiTest.setContentWithTestProviders(content: @Composable () -> Unit) {
    setContent {
        TestWindowInfoProvider(content)
    }
}

@Composable
fun TestWindowInfoProvider(content: @Composable () -> Unit) {
    val windowInfo = remember { TestWindowInfo() }
    currentTestWindowInfo = windowInfo
    CompositionLocalProvider(LocalWindowInfo provides windowInfo) {
        content()
    }
}

@OptIn(ExperimentalTestApi::class)
fun ComposeUiTest.captureScreenshot(label: String) {
    val context = currentUiTest.get() ?: error("captureScreenshot must be called within uiTest { }")
    if (System.getenv(SCREENSHOTS_ENV_VAR) == null) return
    saveScreenshot(onRoot().captureToImage(), "${context.name}.${context.screenshotCount++}.$label")
}

internal lateinit var currentTestWindowInfo: TestWindowInfo

internal class TestWindowInfo : WindowInfo {
    override val isWindowFocused = true
    override var keyboardModifiers: PointerKeyboardModifiers by mutableStateOf(PointerKeyboardModifiers())
}

private val currentUiTest = ThreadLocal<UiTestContext>()

private class UiTestContext(
    val name: String,
) {
    var screenshotCount = 0
}

/**
 * Whether to save `captureScreenshot` screenshots when running UI tests.
 */
private const val SCREENSHOTS_ENV_VAR = "STELLAR_UI_TEST_SCREENSHOTS"

internal fun saveScreenshot(
    bitmap: ImageBitmap,
    name: String,
    directory: String = "build/test-results/screenshots",
) {
    val bytes =
        Image
            .makeFromBitmap(bitmap.asSkiaBitmap())
            .encodeToData(EncodedImageFormat.PNG)!!
            .bytes
    val outFile = File("$directory/$name.png")
    outFile.parentFile.mkdirs()
    outFile.writeBytes(bytes)
    println("Saved screenshot to ${outFile.absolutePath}")
}
