package net.trillia.stellar.desktop.table

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * Draws the table's background stripes.
 */
@Composable
fun TableStripes(
    listState: LazyListState,
    selectedItemListIndex: Int? = null,
) {
    val rowHeightPx = with(LocalDensity.current) { tableRowHeight.toPx() }

    Box(
        Modifier
            .fillMaxSize()
            .drawBehind {
                var itemIndex = listState.firstVisibleItemIndex
                var offsetY = -listState.firstVisibleItemScrollOffset.toFloat()
                while (offsetY < size.height) {
                    val color =
                        when {
                            itemIndex == selectedItemListIndex -> tableRowColorSelected
                            itemIndex % 2 == 1 -> tableRowColorSecondary
                            else -> tableRowColorPrimary
                        }
                    drawRect(
                        color = color,
                        topLeft = Offset(0f, offsetY.coerceAtLeast(0f)),
                        size = Size(size.width, rowHeightPx + 1),
                    )
                    itemIndex += 1
                    offsetY += rowHeightPx
                }
            },
    )
}

@Composable
@Preview
fun TableStripesPreview() {
    Box(
        Modifier.size(100.dp),
    ) {
        TableStripes(rememberLazyListState())
    }
}
