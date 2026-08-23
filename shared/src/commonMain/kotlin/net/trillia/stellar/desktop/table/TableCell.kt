package net.trillia.stellar.desktop.table

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import org.jetbrains.compose.resources.Font
import stellar.shared.generated.resources.Res
import stellar.shared.generated.resources.tahoma

@Composable
fun <Row, Value> TableCell(
    column: TableColumnDefinition<Row, Value>,
    row: Row,
) {
    column.renderer(column.accessor(row))
}

@Composable
fun TableCellText(value: String) {
    val tahoma = FontFamily(Font(Res.font.tahoma))
    Text(
        value,
        fontFamily = tahoma,
        fontSize = 14.sp,
        fontWeight = FontWeight.Medium,
        lineHeight = 1.em,
        maxLines = 1,
        overflow = TextOverflow.MiddleEllipsis,
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(start = tableCellPaddingStart),
    )
}

val tableCellPaddingStart = 3.dp
