package com.cyhdev.minecraft.ask;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.StringReader;

/** Synthetic formatting and pipe-boundary checks; no Minecraft or Codex process. */
public final class BridgeClientTest {
    private BridgeClientTest() {}

    public static void main(String[] args) throws IOException {
        String text = BridgeClient.plain("§\u202eReason\n" + "🪨 word ".repeat(1000));
        check(!text.contains("\n") && !text.contains("§") && !text.contains("\u202e"));
        check(text.codePointCount(0, text.length()) <= 360);
        check(text.split(" ").length <= 60);
        check(BridgeClient.readLine(new BufferedReader(new StringReader("ok\n"))).equals("ok"));
        for (String invalid : new String[] {"missing newline", "a".repeat(4096) + "\n"}) {
            boolean rejected = false;
            try { BridgeClient.readLine(new BufferedReader(new StringReader(invalid))); }
            catch (IOException expected) { rejected = true; }
            check(rejected);
        }
        System.out.println("BridgeClient: 6 checks passed");
    }

    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("Bridge boundary assertion failed");
    }
}
