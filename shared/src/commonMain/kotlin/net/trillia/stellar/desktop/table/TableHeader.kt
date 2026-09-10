package net.trillia.stellar.desktop.table

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import net.trillia.stellar.AppColors
import net.trillia.stellar.FakeHandler

@Composable
fun TableHeader(
    columnState: TableColumnState<*>,
    onColumnTap: (String) -> Unit,
) {
    Box(
        Modifier
            .fillMaxWidth()
            .height(tableRowHeight)
            .background(AppColors.TableHeaderBackground)
            .drawBehind {
                // Draw darkened background for dragged column
                columnState.draggedColumnId?.let { draggedColumnId ->
                    val (originalLeftEdge, originalRightEdge) = columnState.columnEdges(draggedColumnId)
                    drawRect(
                        AppColors.TableHeaderBackgroundDragged,
                        Offset(originalLeftEdge, 0f),
                        Size(originalRightEdge - originalLeftEdge, tableRowHeight.toPx()),
                    )
                }
            },
    ) {
        TableRowLayout(
            columnState = columnState,
            modifier = Modifier.fillMaxWidth(),
        ) {
            columnState.columnOrder.forEach { id ->
                key(id) {
                    val columnDefinition = columnState.columns[id] ?: return@TableRowLayout
                    TableHeaderColumn(
                        sort = columnDefinition.sort,
                        id = id,
                        columnState = columnState,
                        onTap = { onColumnTap(id) },
                    )
                }
            }
        }
    }
}

@Composable
@Preview(widthDp = 200)
fun TableHeaderPreview() {
    val columns =
        listOf(
            TableColumnDefinition<Unit, String>(
                id = "a",
                header = "test",
                initialWidth = 50.dp,
                sort = null,
                accessor = {
                    ""
                },
                renderer = { TableCellText(it) },
            ),
            TableColumnDefinition<Unit, String>(
                id = "b",
                header = "test",
                initialWidth = 50.dp,
                sort = null,
                accessor = {
                    ""
                },
                renderer = { TableCellText(it) },
            ),
        )
    TableHeader(
        rememberTableColumnState(columns),
        FakeHandler("onColumnTap"),
    )
}
