package net.trillia.stellar.desktop.table

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

@Composable
fun TableHeader(fieldSet: FieldSet) {
    Row(Modifier.height(tableRowHeight).fillMaxWidth().background(Color(0xFFDADADA))) {
        fieldSet.fields.forEach {
            TableHeaderColumn(it)
        }
    }
}

@Composable
@Preview(widthDp = 200)
fun TableHeaderPreview() {
    TableHeader(FieldSet(FieldInfo("test", 50.dp), FieldInfo("test", 50.dp)))
}
