package net.trillia.stellar

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ComposeCompilerApi
import androidx.compose.runtime.Composer
import androidx.compose.runtime.currentComposer
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.v2.runComposeUiTest
import io.github.classgraph.ClassGraph
import java.lang.reflect.InvocationTargetException
import java.lang.reflect.Method
import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * Renders every `@Preview` function to check that all previews run.
 *
 * With `STELLAR_RENDER_PREVIEWS` set, each preview is also saved as
 * `build/previews/<source file>/<function>.png`. Set `STELLAR_RENDER_PREVIEWS_FILTER` to only
 * render previews whose name contains it (case-insensitive). `just preview` wraps this.
 */
class RenderPreviewsTest {
    @Test
    fun renderPreviews() {
        val filter = System.getenv(FILTER_ENV_VAR)?.trim().orEmpty()
        val previews = findPreviews().filter { it.name.contains(filter, ignoreCase = true) }
        assertTrue(previews.isNotEmpty(), "No previews match filter '$filter'")

        val failures =
            previews.mapNotNull { preview ->
                try {
                    render(preview)
                    null
                } catch (t: Throwable) {
                    "${preview.name}: ${(t as? InvocationTargetException)?.targetException ?: t}"
                }
            }
        assertTrue(failures.isEmpty(), "Failed to render previews:\n${failures.joinToString("\n")}")
    }
}

/** Matches `STELLAR_RENDER_PREVIEWS` in the `just preview` recipe. */
private const val RENDER_ENV_VAR = "STELLAR_RENDER_PREVIEWS"

/** Matches `STELLAR_RENDER_PREVIEWS_FILTER` in the `just preview` recipe. */
private const val FILTER_ENV_VAR = "STELLAR_RENDER_PREVIEWS_FILTER"

/** Where screenshots go, relative to the module. Must match the `just preview` recipe. */
private const val PREVIEWS_DIR = "build/previews"

private const val PREVIEW_TAG = "preview"

private const val PREVIEW_ANNOTATION = "androidx.compose.ui.tooling.preview.Preview"

private class PreviewFunction(
    val method: Method,
) {
    /** The source file's name, e.g. `ContextMenu` for `ContextMenu.kt`. */
    val sourceName = method.declaringClass.simpleName.removeSuffix("Kt")

    /** Unique among previews, e.g. `ContextMenu.ContextMenuPreview`. */
    val name = "$sourceName.${method.name}"
}

/**
 * Finds `@Preview` functions that take no parameters. `@Preview` has binary retention, so reflection can't see it;
 * ClassGraph reads it from the class files instead.
 */
private fun findPreviews(): List<PreviewFunction> =
    ClassGraph()
        .enableAllInfo()
        .ignoreClassVisibility()
        .ignoreMethodVisibility()
        .acceptPackages("net.trillia.stellar")
        .scan()
        .use { scan ->
            scan
                .getClassesWithMethodAnnotation(PREVIEW_ANNOTATION)
                .flatMap { it.declaredMethodInfo }
                .filter { it.hasAnnotation(PREVIEW_ANNOTATION) }
                .map { it.loadClassAndGetMethod() }
                // The compiler adds `(Composer, Int)` to composable functions; anything else has real parameters.
                .filter { it.parameterTypes.toList() == listOf(Composer::class.java, Int::class.javaPrimitiveType) }
                .map { PreviewFunction(it) }
                .sortedBy { it.name }
        }

@OptIn(ExperimentalTestApi::class)
private fun render(preview: PreviewFunction) {
    runComposeUiTest {
        setContentWithTestProviders {
            Box(Modifier.testTag(PREVIEW_TAG)) {
                InvokePreview(preview.method)
            }
        }
        waitForIdle()

        if (System.getenv(RENDER_ENV_VAR) != null) {
            saveScreenshot(
                onNodeWithTag(PREVIEW_TAG).captureToImage(),
                name = preview.method.name.replace('/', '_'),
                directory = "$PREVIEWS_DIR/${preview.sourceName}",
            )
        }
    }
}

@OptIn(ComposeCompilerApi::class)
@Composable
private fun InvokePreview(method: Method) {
    method.isAccessible = true
    method.invoke(null, currentComposer, 0)
}
