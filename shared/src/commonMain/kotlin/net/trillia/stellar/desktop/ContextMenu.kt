package net.trillia.stellar.desktop

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.isSecondaryPressed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.round
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import net.trillia.stellar.FakeHandler
import net.trillia.stellar.PreviewWrapper
import net.trillia.stellar.runIf

sealed class ContextMenuItem {
    class Action(
        val label: String,
        val onClick: () -> Unit,
    ) : ContextMenuItem()

    object Divider : ContextMenuItem()
}

@Composable
fun ContextMenu(
    items: List<ContextMenuItem>,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    ContextMenu(
        onContextMenuRequest = { items },
        modifier,
        content,
    )
}

@Composable
fun ContextMenu(
    onContextMenuRequest: () -> List<ContextMenuItem>,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    var coordinates by remember { mutableStateOf<LayoutCoordinates?>(null) }

    // In window coordinates, captured at click time so the menu stays put if the content moves.
    var clickPosition by remember { mutableStateOf<IntOffset?>(null) }
    var menuItems by remember { mutableStateOf<List<ContextMenuItem>>(emptyList()) }

    val handleDismiss = {
        clickPosition = null
    }

    Box(
        modifier
            .onGloballyPositioned { coordinates = it }
            .pointerInput(Unit) {
                awaitEachGesture {
                    val event = awaitPointerEvent()
                    if (event.type == PointerEventType.Press && event.buttons.isSecondaryPressed) {
                        val items = onContextMenuRequest()
                        if (items.isNotEmpty()) {
                            val localPosition = event.changes.firstOrNull()?.position
                            menuItems = items
                            clickPosition = localPosition?.let { coordinates?.localToWindow(it)?.round() }
                        }
                    }
                }
            },
    ) {
        content()

        clickPosition?.let { clickPosition ->
            Popup(
                popupPositionProvider = remember(clickPosition) { WindowPositionProvider(clickPosition) },
                onDismissRequest = handleDismiss,
            ) {
                ContextMenuContent(menuItems, handleDismiss)
            }
        }
    }
}

/**
 * Places a popup's top-left corner at [position] in window coordinates, shifted to stay within the
 * window.
 */
private class WindowPositionProvider(
    private val position: IntOffset,
) : PopupPositionProvider {
    override fun calculatePosition(
        anchorBounds: IntRect,
        windowSize: IntSize,
        layoutDirection: LayoutDirection,
        popupContentSize: IntSize,
    ): IntOffset =
        IntOffset(
            position.x.coerceIn(0, (windowSize.width - popupContentSize.width).coerceAtLeast(0)),
            position.y.coerceIn(0, (windowSize.height - popupContentSize.height).coerceAtLeast(0)),
        )
}

private val MENU_MIN_WIDTH = 200.dp

@Composable
internal fun ContextMenuContent(
    items: List<ContextMenuItem>,
    onDismiss: () -> Unit,
) {
    Column(
        Modifier
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .widthIn(min = MENU_MIN_WIDTH)
            .width(IntrinsicSize.Max)
            .verticalScroll(rememberScrollState()),
    ) {
        items.forEach { item ->
            when (item) {
                is ContextMenuItem.Action -> {
                    ContextMenuAction(item, onDismiss)
                }

                is ContextMenuItem.Divider -> {
                    ContextMenuDivider()
                }
            }
        }
    }
}

@Composable
internal fun ContextMenuAction(
    item: ContextMenuItem.Action,
    onDismiss: () -> Unit,
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isHovered by interactionSource.collectIsHoveredAsState()

    Row(
        Modifier
            .height(24.dp)
            .fillMaxWidth()
            .clickable {
                item.onClick()
                onDismiss()
            }.hoverable(interactionSource)
            .runIf(isHovered) {
                background(MaterialTheme.colorScheme.surfaceVariant)
            }.padding(2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(item.label, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
internal fun ContextMenuDivider() {
    HorizontalDivider()
}

@Composable
@Preview
private fun `Preview ContextMenu Default`() {
    PreviewWrapper(padded = true) {
        Box(
            Modifier.size(300.dp),
        ) {
            ContextMenu(previewItems) {
                Text("context menu")
            }
        }
    }
}

@Composable
@Preview
private fun `Preview ContextMenuContent Default`() {
    PreviewWrapper {
        ContextMenuContent(previewItems, FakeHandler("onDismiss"))
    }
}

@Composable
@Preview
private fun `Preview ContextMenuContent Tall`() {
    PreviewWrapper {
        // Limit the size to simulate the window being small
        Box(Modifier.sizeIn(maxWidth = 200.dp, maxHeight = 200.dp)) {
            ContextMenuContent(
                items = List(40) { ContextMenuItem.Action("item $it", FakeHandler("onClick")) },
                onDismiss = FakeHandler("onDismiss"),
            )
        }
    }
}

@Composable
@Preview
private fun `Preview ContextMenuContent Long label`() {
    PreviewWrapper {
        // Limit the size to simulate the window being small
        Box(Modifier.sizeIn(maxWidth = 200.dp, maxHeight = 200.dp)) {
            ContextMenuContent(
                items =
                    listOf(
                        ContextMenuItem.Action("short label", FakeHandler("onClick")),
                        ContextMenuItem.Action(
                            "long label long label long label long label long label long label",
                            FakeHandler("onClick"),
                        ),
                    ),
                onDismiss = FakeHandler("onDismiss"),
            )
        }
    }
}

private val previewItems =
    listOf(
        ContextMenuItem.Action("one", FakeHandler("onClick")),
        ContextMenuItem.Action("two", FakeHandler("onClick")),
        ContextMenuItem.Divider,
        ContextMenuItem.Action("three", FakeHandler("onClick")),
    )
