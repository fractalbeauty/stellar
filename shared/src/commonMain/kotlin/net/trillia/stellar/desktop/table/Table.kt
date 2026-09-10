package net.trillia.stellar.desktop.table

import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.mutableStateSetOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.isShiftPressed
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import net.trillia.stellar.FakeHandler
import net.trillia.stellar.isCtrlLikePressed
import net.trillia.stellar.upOrDownTo
import kotlin.math.roundToInt

@Composable
fun <Row> Table(
    data: List<Row>,
    columns: List<TableColumnDefinition<Row, *>>,
    isRowSelected: (Row) -> Boolean,
    onSelectRow: (Row) -> Unit,
    onDeselectRow: (Row) -> Unit,
    onDeselectAllRows: () -> Unit,
    onColumnTap: (String) -> Unit,
) {
    val density = LocalDensity.current

    val keyboardModifiers by rememberUpdatedState(LocalWindowInfo.current.keyboardModifiers)

    val columnState = rememberTableColumnState(columns)
    val listState = rememberLazyListState()

    val backgroundInteractionSource = remember { MutableInteractionSource() }

    val totalWidthDp = with(density) { columnState.totalWidthPx.toDp() }

    var lastClickedRowIndex by remember { mutableStateOf(0) }
    var lastAnchoredRowIndex by remember { mutableStateOf(0) }

    BoxWithConstraints(
        modifier =
            Modifier
                .fillMaxSize()
                .combinedClickable(
                    onClick = { onDeselectAllRows() },
                    interactionSource = backgroundInteractionSource,
                ),
    ) {
        // Render with the larger of the width of the columns or the width available, so the table
        // fills the available space.
        val contentWidthDp = maxOf(totalWidthDp, maxWidth)

        // Add one to the selected index for the sticky header
        TableStripes(data, isRowSelected, listState)

        Box(Modifier.fillMaxSize().horizontalScroll(columnState.horizontalScrollState)) {
            Box(Modifier.width(contentWidthDp).fillMaxHeight()) {
                LazyColumn(state = listState) {
                    stickyHeader {
                        TableHeader(
                            columnState = columnState,
                            onColumnTap = onColumnTap,
                        )
                    }

                    itemsIndexed(data) { rowIndex, row ->
                        TableRow(
                            columnState = columnState,
                            row = row,
                            onSelected = {
                                when {
                                    keyboardModifiers.isCtrlLikePressed -> {
                                        if (isRowSelected(row)) {
                                            onDeselectRow(row)
                                        } else {
                                            onSelectRow(row)
                                        }

                                        lastClickedRowIndex = rowIndex
                                        lastAnchoredRowIndex = rowIndex
                                    }

                                    keyboardModifiers.isShiftPressed -> {
                                        for (index in rowIndex upOrDownTo lastClickedRowIndex) {
                                            onDeselectRow(data[index])
                                        }
                                        for (index in rowIndex upOrDownTo lastAnchoredRowIndex) {
                                            onSelectRow(data[index])
                                        }

                                        lastClickedRowIndex = rowIndex
                                    }

                                    else -> {
                                        onDeselectAllRows()
                                        onSelectRow(row)

                                        lastClickedRowIndex = rowIndex
                                        lastAnchoredRowIndex = rowIndex
                                    }
                                }
                            },
                        )
                    }
                }

                TableColumnDividers(columnState)
            }
        }
    }
}

/**
 * Layout that places its children according to [columnState], assuming one child is rendered for
 * each column in the state.
 */
@Composable
internal fun TableRowLayout(
    columnState: TableColumnState<*>,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Layout(
        content = content,
        modifier = modifier,
    ) { measurables, constraints ->
        val order = columnState.columnOrder
        val widthsPx = IntArray(order.size)

        var columnTotalWidth = 0
        for (columnIndex in order.indices) {
            val columnWidth = (columnState.columnWidths[order[columnIndex]] ?: 0f).roundToInt().coerceAtLeast(0)
            widthsPx[columnIndex] = columnWidth
            columnTotalWidth += columnWidth
        }

        // Assume `measurables` (layout children) are always in `columnOrder` order
        val placed = measurables.mapIndexed { columnIndex, measurable -> measurable.measure(Constraints.fixedWidth(widthsPx[columnIndex])) }
        val height = placed.maxOfOrNull { it.height } ?: 0

        // Respect minWidth constraint. This ensures fillMaxWidth actually fills available space.
        val totalWidth = columnTotalWidth.coerceAtLeast(constraints.minWidth)

        layout(totalWidth, height) {
            var x = 0
            for (columnIndex in order.indices) {
                val columnOffset = columnState.columnOffset(order[columnIndex])
                val columnX = (x + columnOffset).coerceAtLeast(0f)
                placed[columnIndex].placeRelative(columnX.roundToInt(), 0)
                x += widthsPx[columnIndex]
            }
        }
    }
}

const val TABLE_DRAG_ALPHA = 0.3f

@Composable
@Preview
fun TablePreview() {
    val columns =
        listOf(
            TableColumnDefinition<Map<String, String>, String>(
                id = "a",
                header = "Column A",
                initialWidth = 100.dp,
                sort = null,
                accessor = {
                    it["a"].orEmpty()
                },
                renderer = @Composable { value -> TableCellText(value) },
            ),
            TableColumnDefinition<Map<String, String>, String>(
                id = "b",
                header = "Column B",
                initialWidth = 100.dp,
                sort = null,
                accessor = { it["b"].orEmpty() },
                renderer = @Composable { value -> TableCellText(value) },
            ),
        )

    val data =
        buildList {
            repeat(100) {
                add(mapOf("a" to "aaa", "b" to "bbb"))
            }
        }

    val selected = remember { mutableStateSetOf<Map<String, String>>() }

    Box(modifier = Modifier.size(300.dp, 200.dp)) {
        Table(
            data,
            columns,
            { selected.contains(it) },
            { selected.add(it) },
            { selected.remove(it) },
            onDeselectAllRows = { selected.clear() },
            onColumnTap = FakeHandler("onColumnTap"),
        )
    }
}
