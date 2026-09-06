package net.trillia.stellar

import androidx.compose.ui.input.pointer.PointerKeyboardModifiers
import androidx.compose.ui.input.pointer.isCtrlPressed
import androidx.compose.ui.input.pointer.isMetaPressed

/**
 * Returns whether Command (on Mac) or Ctrl (on other platforms) is pressed.
 */
val PointerKeyboardModifiers.isCtrlLikePressed: Boolean
    get() =
        if (getPlatform().isMac) {
            isMetaPressed
        } else {
            isCtrlPressed
        }
