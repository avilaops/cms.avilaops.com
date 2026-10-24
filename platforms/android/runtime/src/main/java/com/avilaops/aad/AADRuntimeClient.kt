package com.avilaops.aad

import java.net.URI
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** Transport is injected so authentication and HTTP policy can be tested without network. */
fun interface AADHttpTransport {
    fun execute(request: AADHttpRequest): AADHttpResponse
}

data class AADHttpRequest(val url: String, val headers: Map<String, String>)
data class AADHttpResponse(val status: Int, val body: ByteArray, val etag: String?)

sealed interface AADFetchResult {
    data class Updated(val definition: AADApp, val etag: String?) : AADFetchResult
    data object NotModified : AADFetchResult
}

class AADRuntimeClient(
    private val baseUrl: String,
    private val appId: String,
    private val environment: String,
    private val accessToken: () -> String?,
    private val transport: AADHttpTransport,
) {
    private val json = Json { ignoreUnknownKeys = false }

    /** Never sends Bearer credentials to HTTP URLs or userinfo-bearing URLs. */
    fun fetch(etag: String? = null): AADFetchResult {
        val base = try { URI(baseUrl) } catch (_: Exception) {
            throw AADValidationException("Invalid HTTPS base URL")
        }
        if (base.scheme?.lowercase() != "https" || base.host.isNullOrBlank() ||
            base.userInfo != null || base.query != null || base.fragment != null
        ) throw AADValidationException("Invalid HTTPS base URL")
        val slug = Regex("[a-z][a-z0-9-]{0,63}")
        if (!slug.matches(appId) || environment !in setOf("development", "staging", "production"))
            throw AADValidationException("Invalid app or environment")
        val token = accessToken()?.takeIf { it.isNotBlank() }
            ?: throw AADValidationException("Authentication required")
        val endpoint = URI(
            "https", null, base.host, base.port,
            (base.path.trimEnd('/') + "/api/runtime/v1/apps/$appId/environments/$environment"),
            null, null
        )
        val headers = mutableMapOf(
            "Authorization" to "Bearer $token",
            "Accept" to "application/json"
        )
        if (etag != null) headers["If-None-Match"] = etag
        val response = transport.execute(AADHttpRequest(endpoint.toASCIIString(), headers))
        if (response.status == 304) return AADFetchResult.NotModified
        if (response.status != 200) throw AADValidationException("AAD request failed")
        val envelope = json.decodeFromString<AADPublishedEnvelope>(
            response.body.toString(Charsets.UTF_8)
        )
        if (envelope.appId != appId || envelope.environment != environment ||
            envelope.version <= 0 || envelope.definition.appId != appId
        ) throw AADValidationException("Published definition mismatch")
        AADDecoder.validate(envelope.definition)
        return AADFetchResult.Updated(envelope.definition, response.etag)
    }
}

@Serializable
private data class AADPublishedEnvelope(
    val appId: String,
    val environment: String,
    val version: Int,
    val definition: AADApp,
)
