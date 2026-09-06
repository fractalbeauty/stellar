package net.trillia.stellar

import platform.Foundation.NSNumber
import platform.Foundation.NSNumberFormatter
import platform.UIKit.UIDevice

class IOSPlatform : Platform {
    override val name: String = UIDevice.currentDevice.systemName() + " " + UIDevice.currentDevice.systemVersion

    override val isMac: Boolean = false
}

actual fun getPlatform(): Platform = IOSPlatform()

actual fun formatFloat(
    float: Float,
    decimals: Int,
): String {
    val formatter = NSNumberFormatter()
    formatter.minimumFractionDigits = 0u
    formatter.maximumFractionDigits = decimals.toULong()
    formatter.numberStyle = 1u // Decimal
    return formatter.stringFromNumber(NSNumber(float))!!
}
