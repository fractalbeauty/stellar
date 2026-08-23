package net.trillia.stellar.desktop.table

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.trillia.stellar.fieldPadStart
import org.jetbrains.compose.resources.Font
import stellar.shared.generated.resources.Res
import stellar.shared.generated.resources.tahoma

@Composable
fun TableColumnHeader(fieldInfo: FieldInfo) {
    val tahoma = FontFamily(Font(Res.font.tahoma))
    Box(Modifier.background(Color(0xFFDADADA))) {
        Text(
            fieldInfo.label,
            fontFamily = tahoma,
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            lineHeight = 1.em,
            maxLines = 1,
            overflow = TextOverflow.MiddleEllipsis,
            modifier =
                Modifier
                    .width(fieldInfo.width)
                    .padding(vertical = 2.dp)
                    .padding(start = fieldPadStart),
        )
    }
}
