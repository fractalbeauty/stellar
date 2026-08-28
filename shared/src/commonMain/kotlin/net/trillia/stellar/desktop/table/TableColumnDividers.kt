package net.trillia.stellar.desktop.table

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/**
 * Draws ticks in the header between columns, and vertical dividers to the left and right of the
 * column currently being dragged.
 */
@Composable
fun TableColumnDividers(
    columnState: TableColumnState<*>,
    modifier: Modifier = Modifier,
) {
    val draggedColumnId = columnState.draggedColumnId
    Box(
        modifier.fillMaxSize().drawBehind {
            for (columnId in columnState.columnOrder) {
                val (originalLeftEdge, originalRightEdge) = columnState.columnEdges(columnId)
                val offset = columnState.columnOffset(columnId)
                val width = columnState.columnWidths[columnId] ?: 0f

                val draggedLeftEdge = (originalLeftEdge + offset).coerceAtLeast(0f)
                val draggedRightEdge = (originalRightEdge + offset).coerceAtLeast(width)

                // Draw ticks
                drawLine(
                    columnDividerColor,
                    Offset(originalLeftEdge, tableHeaderTickStartY.toPx()),
                    Offset(originalLeftEdge, tableHeaderTickEndY.toPx()),
                    strokeWidth = 1f,
                )
                drawLine(
                    columnDividerColor,
                    Offset(originalRightEdge, tableHeaderTickStartY.toPx()),
                    Offset(originalRightEdge, tableHeaderTickEndY.toPx()),
                    strokeWidth = 1f,
                )

                // Draw dividers when dragging
                if (columnId == draggedColumnId) {
                    drawLine(
                        columnDividerColor,
                        Offset(draggedLeftEdge, 0f),
                        Offset(draggedLeftEdge, size.height),
                        strokeWidth = 1f,
                    )
                    drawLine(
                        columnDividerColor,
                        Offset(draggedRightEdge, 0f),
                        Offset(draggedRightEdge, size.height),
                        strokeWidth = 1f,
                    )
                }
            }
        },
    )
}

private val columnDividerColor = Color.Black.copy(alpha = 0.15f)

private val tableHeaderTickStartY = 8.dp
private val tableHeaderTickEndY = tableRowHeight
