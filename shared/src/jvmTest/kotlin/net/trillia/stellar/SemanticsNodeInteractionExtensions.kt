package net.trillia.stellar

import androidx.compose.ui.input.pointer.PointerKeyboardModifiers
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.performClick

fun SemanticsNodeInteraction.performCtrlClick() =
    withModifiers(
        if (getPlatform().isMac) {
            PointerKeyboardModifiers(isMetaPressed = true)
        } else {
            PointerKeyboardModifiers(isCtrlPressed = true)
        },
    )

fun SemanticsNodeInteraction.performShiftClick() = withModifiers(PointerKeyboardModifiers(isShiftPressed = true))

fun SemanticsNodeInteraction.performCtrlShiftClick() =
    withModifiers(
        if (getPlatform().isMac) {
            PointerKeyboardModifiers(isMetaPressed = true, isShiftPressed = true)
        } else {
            PointerKeyboardModifiers(isCtrlPressed = true, isShiftPressed = true)
        },
    )

private fun SemanticsNodeInteraction.withModifiers(modifiers: PointerKeyboardModifiers): SemanticsNodeInteraction {
    currentTestWindowInfo.keyboardModifiers = modifiers
    try {
        performClick()
    } finally {
        currentTestWindowInfo.keyboardModifiers = PointerKeyboardModifiers()
    }
    return this
}
