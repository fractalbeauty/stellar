package net.trillia.stellar.desktop.table

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.layoutId
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.trillia.stellar.mapIf
import net.trillia.stellar.pointerInputHorizontalDrag
import org.jetbrains.compose.resources.Font
import stellar.shared.generated.resources.Res
import stellar.shared.generated.resources.tahoma

@Composable
fun TableHeaderColumn(
    columnState: TableColumnState<*>,
    id: String,
) {
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()

    val tahoma = FontFamily(Font(Res.font.tahoma))

    val columnDefinition = columnState.columns.getValue(id)

    val minWidthPx = with(density) { columnDefinition.minWidth.toPx() }
    val widthPx = columnState.columnWidths[id] ?: minWidthPx
    val widthDp = with(density) { widthPx.toDp() }

    Box(modifier = Modifier.layoutId(id).width(widthDp).height(tableRowHeight)) {
        Box(
            contentAlignment = Alignment.CenterStart,
        ) {
            Text(
                columnDefinition.header,
                fontFamily = tahoma,
                fontSize = 14.sp,
                fontWeight = FontWeight.Medium,
                lineHeight = 1.em,
                maxLines = 1,
                overflow = TextOverflow.MiddleEllipsis,
                modifier =
                    Modifier
                        .width(widthDp)
                        .padding(start = tableCellPaddingStart),
            )
        }

        // Box for reorder drag
        Box(
            modifier =
                Modifier
                    .align(Alignment.CenterStart)
                    // Offset by half the resize handle width so they don't overlap
                    .offset(x = resizeHandleWidth / 2)
                    .width(widthDp - resizeHandleWidth)
                    .height(tableRowHeight)
                    .mapIf(DEBUG_REORDER_DRAG) { it.background(Color.Blue.copy(alpha = 0.5f)) }
                    .pointerInputHorizontalDrag(
                        key = id,
                        onDragStart = { columnState.handleReorderDragStart(id) },
                        onDragEnd = { columnState.handleReorderDragEnd(id, scope) },
                        onDrag = { deltaX -> columnState.handleReorderDragMove(id, deltaX, scope) },
                    ),
        )

        // Box for resize handle
        Box(
            modifier =
                Modifier
                    .align(Alignment.CenterEnd)
                    .offset(x = resizeHandleWidth / 2)
                    .width(resizeHandleWidth)
                    .height(tableRowHeight)
                    .mapIf(DEBUG_RESIZE_DRAG) { it.background(Color.Red) }
                    .pointerInputHorizontalDrag(
                        key = id,
                        onDragStart = {},
                        onDragEnd = {},
                        onDrag = { deltaX -> columnState.handleResizeDragMove(id, deltaX, density) },
                    ),
        )
    }
}

private const val DEBUG_REORDER_DRAG = false
private const val DEBUG_RESIZE_DRAG = false

private val resizeHandleWidth = 24.dp
