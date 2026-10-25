package net.trillia.stellar.desktop.inspector

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em

internal val inspectorFieldLabelHeight = 24.dp
internal val inspectorFieldHeight = 32.dp
internal val inspectorFieldWidth = 200.dp

@Composable
fun Inspector(
    modifier: Modifier = Modifier,
    fields: @Composable ColumnScope.() -> Unit,
) {
    Column(modifier.padding(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        fields()

        // dummy field
        Box(
            contentAlignment = Alignment.Center,
            modifier =
                Modifier
                    .padding(top = inspectorFieldLabelHeight)
                    .height(inspectorFieldHeight)
                    .width(inspectorFieldWidth)
                    .background(Color(0xFFCCCCCC)),
        ) {
            Text("+", fontWeight = FontWeight.Black, color = Color.White, fontSize = 1.em, lineHeight = 1.em)
        }
    }
}
