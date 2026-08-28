package net.trillia.stellar.desktop.table

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color

/**
 * Draws vertical dividers to the left and right of the column currently being dragged.
 */
@Composable
fun TableColumnDividers(
    columnState: TableColumnState<*>,
    modifier: Modifier = Modifier,
) {
    val draggedColumnId = columnState.draggedColumnId
    Box(
        modifier.fillMaxSize().drawBehind {
            if (draggedColumnId == null) return@drawBehind

            val (originalLeftEdge, originalRightEdge) = columnState.columnEdges(draggedColumnId)
            val offset = columnState.columnOffset(draggedColumnId)
            val width = columnState.columnWidths[draggedColumnId] ?: 0f

            val leftEdge = (originalLeftEdge + offset).coerceAtLeast(0f)
            val rightEdge = (originalRightEdge + offset).coerceAtLeast(width)

            drawLine(
                columnDividerColor,
                Offset(leftEdge, 0f),
                Offset(leftEdge, size.height),
                strokeWidth = 1f,
            )
            drawLine(
                columnDividerColor,
                Offset(rightEdge, 0f),
                Offset(rightEdge, size.height),
                strokeWidth = 1f,
            )
        },
    )
}

private val columnDividerColor = Color.Black.copy(alpha = 0.15f)
