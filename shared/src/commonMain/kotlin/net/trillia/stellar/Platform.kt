package net.trillia.stellar

interface Platform {
    val name: String
}

expect fun getPlatform(): Platform

expect fun formatFloat(
    float: Float,
    decimals: Int,
): String
