package net.trillia.stellar

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.changedToUp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Minimum drag distance before starting a drag gesture.
 */
private val minDragDistance: Dp = 8.dp

/**
 * Detects a horizontal drag gesture or tap gesture, with a minimum drag distance before
 * starting a drag gesture.
 */
fun Modifier.pointerInputHorizontalDrag(
    key: String,
    onTap: () -> Unit = {},
    onDragStart: () -> Unit,
    onDragEnd: () -> Unit,
    onDrag: (deltaX: Float) -> Unit,
): Modifier =
    this.then(
        Modifier.pointerInput(key) {
            val minDragDistancePx = minDragDistance.toPx()

            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                
                var dragOffset = Offset.Zero
                var dragStarted = false

                while (true) {
                    val event = awaitPointerEvent()
                    val change = event.changes.firstOrNull { it.id == down.id } ?: break

                    if (change.changedToUp()) {
                        if (dragStarted) onDragEnd() else onTap()
                        break
                    }

                    val delta = change.positionChange()
                    if (delta != Offset.Zero) {
                        if (dragStarted) {
                            change.consume()
                            onDrag(delta.x)
                        } else {
                            dragOffset += delta
                            if (dragOffset.getDistance() >= minDragDistancePx) {
                                dragStarted = true
                                change.consume()
                                onDragStart()
                                onDrag(dragOffset.x)
                            }
                        }
                    }
                }
            }
        },
    )
