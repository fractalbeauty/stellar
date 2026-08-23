package net.trillia.stellar.desktop.table

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

@Composable
fun TableHeader(columnState: TableColumnState<*>) {
    Box(
        Modifier.height(tableRowHeight).fillMaxWidth(),
    ) {
        TableRowLayout(
            columnState = columnState,
            modifier = Modifier.fillMaxWidth(),
        ) {
            columnState.columnOrder.forEach { id ->
                key(id) {
                    TableHeaderColumn(columnState, id)
                }
            }
        }
    }
}

val tableHeaderBackgroundColor = Color(0xFFDADADA)

@Composable
@Preview(widthDp = 200)
fun TableHeaderPreview() {
    val columns =
        listOf(
            TableColumnDefinition<Unit, String>(id = "a", header = "test", initialWidth = 50.dp, accessor = {
                ""
            }, renderer = { TableCellText(it) }),
            TableColumnDefinition<Unit, String>(id = "b", header = "test", initialWidth = 50.dp, accessor = {
                ""
            }, renderer = { TableCellText(it) }),
        )
    TableHeader(rememberTableColumnState(columns))
}
