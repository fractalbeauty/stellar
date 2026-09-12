package net.trillia.stellar

import androidx.compose.ui.graphics.asSkiaBitmap
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

    return runComposeUiTest {
        try {
            block()
        } catch (t: Throwable) {
            val bitmap = onRoot().captureToImage()
            val bytes =
                Image
                    .makeFromBitmap(bitmap.asSkiaBitmap())
                    .encodeToData(EncodedImageFormat.PNG)!!
                    .bytes
            val outFile = File("build/test-results/screenshots/$name.png")
            outFile.parentFile.mkdirs()
            outFile.writeBytes(bytes)
            println("Saved failure screenshot to ${outFile.absolutePath}")
            throw t
        }
    }
}
