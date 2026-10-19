package com.avilaops.aad

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

@Serializable
data class AADApp(
    val schemaVersion: String,
    val appId: String,
    val minimumRuntimeVersion: String,
    val navigation: AADNavigation,
    val screens: List<AADScreen>,
)

@Serializable
data class AADNavigation(val initialRoute: String)

@Serializable
data class AADScreen(
    val id: String,
    val route: String,
    val title: String,
    val components: List<AADComponent>,
)

@Serializable
sealed interface AADComponent {
    val id: String

    @Serializable
    @SerialName("text")
    data class Text(override val id: String, val value: String) : AADComponent

    @Serializable
    @SerialName("button")
    data class Button(
        override val id: String,
        val label: String,
        val action: AADAction,
    ) : AADComponent

    @Serializable
    @SerialName("spacer")
    data class Spacer(override val id: String) : AADComponent
}

@Serializable
sealed interface AADAction {
    @Serializable
    @SerialName("navigate")
    data class Navigate(val target: String) : AADAction
}

class AADValidationException(message: String) : IllegalArgumentException(message)

/** Immutable AAD model only. No network, scripts, business logic or executable payloads. */
object AADDecoder {
    const val MAX_BYTES = 1024 * 1024

    private val parser = Json {
        ignoreUnknownKeys = false
        isLenient = false
        classDiscriminator = "type"
    }
    private val idRegex = Regex("[a-z][a-z0-9-]{0,63}")

    fun decode(payload: ByteArray): AADApp {
        if (payload.size > MAX_BYTES) throw AADValidationException("Payload exceeds limit")
        val app = parser.decodeFromString<AADApp>(payload.toString(Charsets.UTF_8))
        validate(app)
        return app
    }

    fun validate(app: AADApp) {
        if (app.schemaVersion != "1.0.0" || app.minimumRuntimeVersion != "1.0.0")
            throw AADValidationException("Unsupported schema/runtime")
        if (!idRegex.matches(app.appId)) throw AADValidationException("Invalid app ID")
        if (app.screens.size !in 1..100) throw AADValidationException("Invalid screen count")
        val screenIds = mutableSetOf<String>()
        val routes = mutableSetOf<String>()
        for (screen in app.screens) {
            if (!idRegex.matches(screen.id) || !idRegex.matches(screen.route))
                throw AADValidationException("Invalid screen ID or route")
            if (!screenIds.add(screen.id) || !routes.add(screen.route))
                throw AADValidationException("Duplicate screen ID or route")
            if (screen.components.size > 200) throw AADValidationException("Too many components")
            val ids = mutableSetOf<String>()
            for (component in screen.components) {
                if (!idRegex.matches(component.id) || !ids.add(component.id))
                    throw AADValidationException("Invalid or duplicate component ID")
            }
        }
        if (app.navigation.initialRoute !in routes)
            throw AADValidationException("Unknown initial route")
        app.screens.flatMap { it.components }.forEach { component ->
            if (component is AADComponent.Button &&
                component.action is AADAction.Navigate &&
                component.action.target !in routes
            ) throw AADValidationException("Unknown navigation target")
        }
    }
}
