package com.avilaops.aad

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class AADRuntimeClientTest {
    private val definition = """
        {"schemaVersion":"1.0.0","appId":"tms","minimumRuntimeVersion":"1.0.0",
        "navigation":{"initialRoute":"home"},"screens":[
        {"id":"home","route":"home","title":"Entregas","components":[]}]}
    """.trimIndent()
    private val envelope = """{"appId":"tms","environment":"staging","version":1,"definition":$definition}"""

    @Test fun fetchesValidatedPublishedDefinition() {
        var request: AADHttpRequest? = null
        val client = AADRuntimeClient("https://cms.example", "tms", "staging", { "token" }) {
            req ->
            request = req
            AADHttpResponse(200, envelope.toByteArray(), "\"revision-1\"")
        }
        val result = client.fetch("\"revision-0\"")
        assertTrue(result is AADFetchResult.Updated)
        assertEquals("tms", (result as AADFetchResult.Updated).definition.appId)
        assertEquals("https://cms.example/api/runtime/v1/apps/tms/environments/staging", request?.url)
        assertEquals("Bearer token", request?.headers?.get("Authorization"))
        assertEquals("\"revision-0\"", request?.headers?.get("If-None-Match"))
    }

    @Test fun notModifiedIsNotTreatedAsEmptyAAD() {
        val client = AADRuntimeClient("https://cms.example", "tms", "staging", { "token" }) {
            AADHttpResponse(304, byteArrayOf(), null)
        }
        assertEquals(AADFetchResult.NotModified, client.fetch("\"revision-1\""))
    }

    @Test fun rejectsInsecureURLBeforeReadingTokenOrSendingRequest() {
        for (url in listOf("http://cms.example", "https://user:pass@cms.example", "file:///tmp/aad")) {
            var credentialReads = 0
            var calls = 0
            val client = AADRuntimeClient(url, "tms", "staging", {
                credentialReads++
                "secret"
            }) {
                calls++
                AADHttpResponse(200, envelope.toByteArray(), null)
            }
            try { client.fetch(); fail("Expected invalid URL") }
            catch (_: AADValidationException) { }
            assertEquals(0, credentialReads)
            assertEquals(0, calls)
        }
    }

    @Test fun rejectsOtherApplicationEnvelope() {
        val client = AADRuntimeClient("https://cms.example", "tms", "staging", { "token" }) {
            AADHttpResponse(200, envelope.replace("\"appId\":\"tms\"", "\"appId\":\"other\"").toByteArray(), null)
        }
        try { client.fetch(); fail("Expected mismatch") }
        catch (_: AADValidationException) { }
    }
}
