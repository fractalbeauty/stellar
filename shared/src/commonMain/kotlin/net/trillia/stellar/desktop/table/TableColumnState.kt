package net.trillia.stellar.desktop.table

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ScrollState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import net.trillia.stellar.sumOfFloat
import kotlin.collections.set

/**
 * State for the columns in a table. Handles dragging, resizing, and animating offsets.
 */
class TableColumnState<Row>(
    initialColumns: List<TableColumnDefinition<Row, *>>,
    initialWidths: Map<String, Float>,
) {
    var columns: Map<String, TableColumnDefinition<Row, *>> by mutableStateOf(initialColumns.associateBy { it.id })

    var columnOrder by mutableStateOf(initialColumns.map { it.id })
    val columnWidths = mutableStateMapOf<String, Float>().apply { putAll(initialWidths) }

    val horizontalScrollState = ScrollState(0)

    val totalWidthPx: Float
        get() = columnOrder.sumOfFloat { (columnWidths[it] ?: 0f) }

    var draggedColumnId by mutableStateOf<String?>(null)
        private set

    /** X-offset in pixels for columns when they're being dragged or animated. */
    private val columnOffsets = mutableStateMapOf<String, Float>()

    /** Active animations for columns when they're being animated. */
    private val columnOffsetAnimations = mutableMapOf<String, Job>()

    private fun cancelOffsetAnimation(id: String) {
        columnOffsetAnimations.remove(id)?.cancel()
    }

    /** Animates [id]'s offset from its current value to [to], updating [columnOffsets] each frame. */
    private fun animateOffsetTo(
        id: String,
        to: Float,
        scope: CoroutineScope,
        onFinished: () -> Unit = {},
    ) {
        cancelOffsetAnimation(id)
        val from = columnOffsets[id] ?: 0f
        columnOffsetAnimations[id] =
            scope.launch {
                Animatable(from).animateTo(to, reorderAnimation) { columnOffsets[id] = value }
                columnOffsetAnimations.remove(id)
                onFinished()
            }
    }

    fun handleReorderDragStart(id: String) {
        draggedColumnId = id
    }

    fun handleReorderDragMove(
        id: String,
        deltaX: Float,
        scope: CoroutineScope,
    ) {
        // Stop animating if animating
        cancelOffsetAnimation(id)

        // Update offset by the drag delta
        val newOffset = (columnOffsets[id] ?: 0f) + deltaX
        columnOffsets[id] = newOffset

        val order = columnOrder
        val draggedIndex = order.indexOf(id)
        if (draggedIndex < 0) return

        val widths = columnWidths
        val draggedWidth = widths[id] ?: return

        val (draggedOriginalLeftEdge, draggedOriginalRightEdge) = columnEdges(id)
        val draggedLeftEdge = draggedOriginalLeftEdge + newOffset
        val draggedRightEdge = draggedOriginalRightEdge + newOffset

        // Check if dragged left, past the previous column
        if (deltaX < 0 && draggedIndex > 0) {
            val leftIndex = draggedIndex - 1
            val leftId = order[leftIndex]
            val leftWidth = widths[leftId] ?: 0f
            val leftCenter = draggedOriginalLeftEdge - leftWidth / 2f

            if (draggedLeftEdge < leftCenter) {
                // Swap order
                val newOrder = columnOrder.toMutableList()
                val tmp = newOrder[draggedIndex]
                newOrder[draggedIndex] = newOrder[leftIndex]
                newOrder[leftIndex] = tmp
                columnOrder = newOrder

                // Update offset
                columnOffsets[id] = newOffset + leftWidth

                // Animate swapped column into place
                columnOffsets[leftId] = -draggedWidth
                animateOffsetTo(leftId, 0f, scope)
            }
        }

        // Check if dragged right, past the next column
        if (deltaX > 0 && draggedIndex < order.size - 1) {
            val rightIndex = draggedIndex + 1
            val rightId = order[rightIndex]
            val rightWidth = widths[rightId] ?: 0f
            val rightCenter = draggedOriginalRightEdge + rightWidth / 2f

            if (draggedRightEdge > rightCenter) {
                // Swap order
                val newOrder = columnOrder.toMutableList()
                val tmp = newOrder[draggedIndex]
                newOrder[draggedIndex] = newOrder[rightIndex]
                newOrder[rightIndex] = tmp
                columnOrder = newOrder

                // Update offset
                columnOffsets[id] = newOffset - rightWidth

                // Animate swapped column into place
                columnOffsets[rightId] = draggedWidth
                animateOffsetTo(rightId, 0f, scope)
            }
        }
    }

    fun handleReorderDragEnd(
        id: String,
        scope: CoroutineScope,
    ) {
        if (draggedColumnId == id) draggedColumnId = null

        // Animate dragged column back into place
        animateOffsetTo(id, 0f, scope) {
            columnOffsets.remove(id)
        }
    }

    fun handleResizeDragMove(
        id: String,
        deltaX: Float,
        density: Density,
    ) {
        val columnDefinition = columns[id] ?: return
        val minWidthPx = with(density) { columnDefinition.minWidth.toPx() }

        val width = columnWidths[id] ?: minWidthPx
        columnWidths[id] = (width + deltaX).coerceAtLeast(minWidthPx)
    }

    /** Update the columns state with new column definitions. */
    fun updateColumns(
        newColumns: List<TableColumnDefinition<Row, *>>,
        density: Density,
    ) {
        columns = newColumns.associateBy { it.id }

        // Early return if unchanged
        val newIds = newColumns.map { it.id }
        if (newIds == columnOrder) return
        val newIdsSet = newIds.toSet()

        val existingIds = columnOrder.toSet()
        val addedIds = newIds.filterNot { it in existingIds }

        // Initialize widths
        for (id in addedIds) {
            val columnDefinition = columns.getValue(id)
            columnWidths[id] = with(density) { columnDefinition.initialWidth.toPx() }
        }

        // Remove removed columns and add new columns
        columnOrder = columnOrder.filter { it in newIdsSet } + addedIds

        // Remove removed columns
        for (id in columnWidths.keys - newIdsSet) columnWidths.remove(id)
        for (id in columnOffsets.keys - newIdsSet) columnOffsets.remove(id)
        if (draggedColumnId != null && draggedColumnId !in newIdsSet) draggedColumnId = null
    }

    /** Returns the left and right edges of a column by [id], not including dragging. */
    fun columnEdges(id: String): Pair<Float, Float> {
        val index = columnOrder.indexOf(id)
        if (index < 0) return 0f to 0f

        val width = columnWidths[id] ?: 0f

        var leftEdge = 0f
        for (i in 0 until index) leftEdge += columnWidths[columnOrder[i]] ?: 0f

        val rightEdge = leftEdge + width

        return leftEdge to rightEdge
    }

    /** Returns the offset of a column by [id]. */
    fun columnOffset(id: String): Float = columnOffsets[id] ?: 0f
}

@Composable
fun <Row> rememberTableColumnState(columns: List<TableColumnDefinition<Row, *>>): TableColumnState<Row> {
    val density = LocalDensity.current
    val state =
        remember {
            val widths = columns.associate { it.id to with(density) { it.initialWidth.toPx() } }
            TableColumnState(columns, widths)
        }
    state.updateColumns(columns, density)
    return state
}

private val reorderAnimation = tween<Float>(180)
