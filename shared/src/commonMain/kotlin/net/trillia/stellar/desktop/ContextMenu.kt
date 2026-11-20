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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
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
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.round
import androidx.compose.ui.window.Popup
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
    var menuPosition by remember { mutableStateOf<IntOffset?>(null) }
    var menuItems by remember { mutableStateOf<List<ContextMenuItem>>(emptyList()) }

    val handleDismiss = {
        menuPosition = null
    }

    Box(
        modifier.pointerInput(Unit) {
            awaitEachGesture {
                val event = awaitPointerEvent()
                if (event.type == PointerEventType.Press && event.buttons.isSecondaryPressed) {
                    val items = onContextMenuRequest()
                    if (items.isNotEmpty()) {
                        menuItems = items
                        menuPosition =
                            event.changes
                                .firstOrNull()
                                ?.position
                                ?.round()
                    }
                }
            }
        },
    ) {
        content()

        menuPosition?.let { currentMenuPosition ->
            Popup(
                offset = currentMenuPosition,
                onDismissRequest = handleDismiss,
            ) {
                ContextMenuContent(menuItems, handleDismiss)
            }
        }
    }
}

@Composable
internal fun ContextMenuContent(
    items: List<ContextMenuItem>,
    onDismiss: () -> Unit,
) {
    Column(
        Modifier
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .widthIn(min = 200.dp)
            .width(IntrinsicSize.Max),
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
        Text(item.label)
    }
}

@Composable
internal fun ContextMenuDivider() {
    HorizontalDivider()
}

@Composable
@Preview
fun ContextMenuPreview() {
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
fun ContextMenuContextPreview() {
    PreviewWrapper {
        ContextMenuContent(previewItems, FakeHandler("onDismiss"))
    }
}

private val previewItems =
    listOf(
        ContextMenuItem.Action("one", FakeHandler("onClick")),
        ContextMenuItem.Action("two", FakeHandler("onClick")),
        ContextMenuItem.Divider,
        ContextMenuItem.Action("three", FakeHandler("onClick")),
    )
