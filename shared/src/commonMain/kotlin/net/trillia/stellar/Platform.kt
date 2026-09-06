package net.trillia.stellar

interface Platform {
    val name: String

    val isMac: Boolean
}

expect fun getPlatform(): Platform

expect fun formatFloat(
    float: Float,
    decimals: Int,
): String
