package com.avilaops.aad

import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Test

class AADDecoderTest {
    private val valid = """
      {"schemaVersion":"1.0.0","appId":"app-exemplo","minimumRuntimeVersion":"1.0.0",
      "navigation":{"initialRoute":"home"},"screens":[
        {"id":"home","route":"home","title":"Início","components":[
          {"type":"text","id":"intro","value":"Bem-vindo"},
          {"type":"button","id":"go-orders","label":"Pedidos",
           "action":{"type":"navigate","target":"orders"}}]},
        {"id":"orders","route":"orders","title":"Pedidos","components":[]}]}
    """.trimIndent()

    @Test fun decodesSharedAADShape() {
        assertEquals(2, AADDecoder.decode(valid.toByteArray()).screens.size)
    }

    @Test fun rejectsUnknownFields() = rejects(valid.replace("\"appId\":", "\"unknown\":true,\"appId\":"))
    @Test fun rejectsUnknownNavigation() = rejects(valid.replace("\"target\":\"orders\"", "\"target\":\"missing\""))
    @Test fun rejectsUnknownComponent() = rejects(valid.replace("\"type\":\"text\"", "\"type\":\"script\""))
    @Test fun rejectsDuplicateScreenRoute() = rejects(valid.replace("\"route\":\"orders\"", "\"route\":\"home\""))
    @Test fun rejectsOversizedPayload() {
        try {
            AADDecoder.decode(ByteArray(AADDecoder.MAX_BYTES + 1))
            fail("Expected rejection")
        } catch (_: AADValidationException) {}
    }

    private fun rejects(json: String) {
        try {
            AADDecoder.decode(json.toByteArray())
            fail("Expected invalid AAD")
        } catch (_: IllegalArgumentException) {}
    }
}
