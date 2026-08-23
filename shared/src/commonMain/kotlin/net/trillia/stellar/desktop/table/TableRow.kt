package net.trillia.stellar.desktop.table

import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color

@Composable
fun TableRow(
    fields: FieldSet,
    row: Map<String, String>,
    selected: Boolean,
    onSelected: () -> Unit,
) {
    val rowInteractionSource = remember { MutableInteractionSource() }
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .height(tableRowHeight)
                .run {
                    if (selected) background(tableRowColorSelected) else this
                }.combinedClickable(onClick = onSelected, interactionSource = rowInteractionSource),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        fields.fields.forEach {
            TableCell(it, row.get(it.label))
        }
    }
}

val tableRowColorPrimary = Color(0xFFFAFAFA)
val tableRowColorSecondary = Color(0xFFEEEEEE)
val tableRowColorSelected = Color(0xFFB0D3FF)
