package net.trillia.stellar.desktop.table

import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

@Composable
fun Table(
    myData: Array<Map<String, String>>,
    fields: FieldSet,
    selected: Int?,
    onSelected: (newValue: Int?) -> Unit,
) {
    val bgInteractionSource = remember { MutableInteractionSource() }

    Box(
        modifier =
            Modifier.combinedClickable(
                onClick = {
                    onSelected(null)
                },
                interactionSource = bgInteractionSource,
            ),
    ) {
        Column {
            // draw headers
            Row {
                fields.fields.forEach {
                    TableColumnHeader(it)
                }
            }

            // draw rows
            myData.forEachIndexed { rowindex, row ->
                val rowInteractionSource = remember { MutableInteractionSource() }
                Row(
                    modifier =
                        Modifier.combinedClickable(onClick = {
                            onSelected(rowindex)
                        }, interactionSource = rowInteractionSource),
                ) {
                    val rowVals = fields.fields.map { row.get(it.label) }
                    rowVals.forEachIndexed { colindex, label ->
                        TableCell(
                            fields.fields[colindex],
                            label,
                            rowindex % 2 == 1,
                            rowindex == selected,
                        )
                    }
                }
            }
        }

        // draw first border
        VerticalDivider(thickness = 1.dp)

        // draw ending dividers
        var widthSoFar = 0.dp
        fields.fields.forEach {
            VerticalDivider(
                thickness = 1.dp,
                modifier =
                    Modifier
                        .offset(x = widthSoFar + it.width)
                        .alpha(0.5F),
            )
            widthSoFar += it.width
        }
    }
}

data class FieldSet(
    val fields: List<FieldInfo>,
) {
    constructor(vararg fieldInfo: FieldInfo) : this(listOf(*fieldInfo))
}

data class FieldInfo(
    val label: String,
    val width: Dp,
)

@Composable
@Preview
fun TablePreview() {
    val fields = FieldSet(FieldInfo("test", 100.dp))
    val data = arrayOf(mapOf("test" to "aaa"), mapOf("test" to "bbb"), mapOf("test" to "ccc"))

    Box(modifier = Modifier.size(200.dp, 200.dp)) {
        Table(data, fields, null, {})
    }
}
