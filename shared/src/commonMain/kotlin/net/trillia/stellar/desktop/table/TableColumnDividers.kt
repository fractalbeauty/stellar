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

            val (leftEdge, rightEdge) = columnState.columnEdges(draggedColumnId)
            val offset = columnState.columnOffsets[draggedColumnId] ?: 0f

            drawLine(
                columnDividerColor,
                Offset(leftEdge + offset, 0f),
                Offset(leftEdge + offset, size.height),
                strokeWidth = 1f,
            )
            drawLine(
                columnDividerColor,
                Offset(rightEdge + offset, 0f),
                Offset(rightEdge + offset, size.height),
                strokeWidth = 1f,
            )
        },
    )
}

private val columnDividerColor = Color.Black.copy(alpha = 0.15f)
