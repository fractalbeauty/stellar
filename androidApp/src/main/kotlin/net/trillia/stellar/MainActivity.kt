package net.trillia.stellar

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()

        val splashScreen = installSplashScreen()

        super.onCreate(savedInstanceState)

        // Show splash screen until core is ready
        val app = application as StellarApplication
        splashScreen.setKeepOnScreenCondition { !app.coreReady.value }

        setContent {
            val coreReady by app.coreReady.collectAsState()
            if (coreReady) {
                App(core = app.core)
            }
        }
    }
}
