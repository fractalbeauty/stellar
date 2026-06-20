package net.trillia.stellar

interface Platform {
    val name: String
}

expect fun getPlatform(): Platform