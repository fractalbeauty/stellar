package net.trillia.stellar

import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput

fun Modifier.pointerInputHorizontalDrag(
    key: String,
    onDragStart: () -> Unit,
    onDragEnd: () -> Unit,
    onDrag: (deltaX: Float) -> Unit,
): Modifier =
    this.then(
        Modifier.pointerInput(key) {
            detectDragGestures(
                onDragStart = { onDragStart() },
                onDragEnd = { onDragEnd() },
                onDragCancel = { onDragEnd() },
                onDrag = { change, dragAmount ->
                    change.consume()
                    onDrag(dragAmount.x)
                },
            )
        },
    )
