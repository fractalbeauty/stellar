package net.trillia.stellar.desktop

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import org.jetbrains.compose.resources.Font
import stellar.shared.generated.resources.Res
import stellar.shared.generated.resources.tahoma

@Composable
fun Button(
    label: String,
    isDefault: Boolean = false,
    onClick: () -> Unit,
) {
    val tahoma = FontFamily(Font(Res.font.tahoma))
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val cBgPressed = Color(0xFF000000)
    val cFgPressed = Color(0xFFFFFFFF)
    val cBgBase = if (isDefault) Color(0xFFBBBBBB) else Color(0xFFECECEC)
    val cBgHl = Color(0xFFEFEFEF)
    val shape = RoundedCornerShape(5.dp)
    val bg = Brush.verticalGradient(listOf(cBgHl, cBgBase), 4f, 15f)
    Row(
        modifier =
            Modifier
                .clip(shape)
                .widthIn(min = 78.dp)
                .padding(2.dp)
                .border(1.dp, Color(0xFF777777), shape = shape)
                .clickable(onClick = onClick, interactionSource = interactionSource)
                .run({
                    if (isPressed) background(cBgPressed, shape) else background(bg, shape)
                }),
        horizontalArrangement = Arrangement.Center,
    ) {
        Text(
            label,
            fontFamily = tahoma,
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            lineHeight = 1.em,
            modifier = Modifier.padding(vertical = 3.dp, horizontal = 7.dp),
            color = if (isPressed) cFgPressed else Color.Unspecified,
        )
    }
}