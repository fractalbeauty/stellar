package net.trillia.stellar.desktop.table

import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.layoutId
import androidx.compose.ui.unit.dp

@Composable
fun <Row> TableRow(
    columnState: TableColumnState<Row>,
    row: Row,
    onSelected: () -> Unit,
) {
    val rowInteractionSource = remember { MutableInteractionSource() }
    Box(
        modifier =
            Modifier
                .fillMaxWidth()
                .height(tableRowHeight)
                .combinedClickable(onClick = onSelected, interactionSource = rowInteractionSource),
    ) {
        TableRowLayout(
            columnState = columnState,
            modifier = Modifier.fillMaxWidth(),
        ) {
            columnState.columnOrder.forEach { id ->
                key(id) {
                    val columnDefinition = columnState.columns.getValue(id)
                    Box(
                        modifier = Modifier.layoutId(id).height(tableRowHeight),
                        contentAlignment = Alignment.CenterStart,
                    ) {
                        TableCell(columnDefinition, row)
                    }
                }
            }
        }
    }
}

val tableRowHeight = 20.dp

val tableRowColorPrimary = Color(0xFFFAFAFA)
val tableRowColorSecondary = Color(0xFFEEEEEE)
val tableRowColorSelected = Color(0xFFB0D3FF)
