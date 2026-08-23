package net.trillia.stellar.desktop.table

import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

@Composable
fun Table(
    data: List<Map<String, String>>,
    fields: FieldSet,
    selected: Int?,
    onSelected: (newValue: Int?) -> Unit,
) {
    val bgInteractionSource = remember { MutableInteractionSource() }
    val listState = rememberLazyListState()

    Box(
        modifier =
            Modifier.combinedClickable(
                onClick = {
                    onSelected(null)
                },
                interactionSource = bgInteractionSource,
            ),
    ) {
        TableStripes(listState)

        LazyColumn(state = listState) {
            // draw header
            stickyHeader {
                TableHeader(fields)
            }

            // draw rows
            itemsIndexed(data) { rowIndex, row ->
                TableRow(
                    fields = fields,
                    row = row,
                    selected = rowIndex == selected,
                    onSelected = { onSelected(rowIndex) },
                )
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

val tableRowHeight = 20.dp

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
    val fields =
        FieldSet(
            FieldInfo("Column A", 100.dp),
            FieldInfo("Column B", 100.dp),
        )

    val data =
        buildList {
            repeat(100) {
                add(mapOf("Column A" to "aaa", "Column B" to "bbb"))
            }
        }

    var selected by remember { mutableStateOf<Int?>(null) }

    Box(modifier = Modifier.size(300.dp, 200.dp)) {
        Table(data, fields, selected, { selected = it })
    }
}
