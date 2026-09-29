package com.cyhdev.minecraft.ask;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.StringReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/** Synthetic formatting and pipe-boundary checks; no Minecraft or Codex process. */
public final class BridgeClientTest {
    private BridgeClientTest() {}

    public static void main(String[] args) throws IOException {
        if (args.length > 0 && args[0].equals("--fixture")) { fixture(); return; }
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
        Path work = Files.createTempDirectory("wiki-ask-test-");
        String classpath = String.join(java.io.File.pathSeparator, Arrays.stream(System.getProperty("java.class.path")
                .split(java.io.File.pathSeparator)).map(p -> Path.of(p).toAbsolutePath().toString()).toList());
        List<String> command = List.of(Path.of(System.getProperty("java.home"), "bin", "java").toString(),
                "-cp", classpath, BridgeClientTest.class.getName(), "--fixture", work.toString());
        try (BridgeClient client = new BridgeClient(command)) {
            client.prepare();
            client.prepare();
            String[] rejected = client.ask("player-id", "Alice", "world-id", "Survival", "first");
            check(rejected[0].equals("At capacity."));
            String[] reply = client.ask("player-id", "Bob", "world-id", "Survival", "second");
            check(reply[0].equals("Bob in Survival; previous requests: 1"));
            check(reply[1].endsWith("/100"));
            client.shutdown();
            boolean stopped = false;
            try { client.prepare(); } catch (IOException expected) { stopped = true; }
            check(stopped);
        } finally { Files.deleteIfExists(work); }
        System.out.println("BridgeClient: 10 checks passed");
    }

    private static void fixture() throws IOException {
        System.out.println("{\"ready\":true,\"protocol\":1}");
        var reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        for (int i = 0; i < 2; i++) {
            JsonObject request = JsonParser.parseString(BridgeClient.readLine(reader)).getAsJsonObject();
            check(request.get("player_uuid").getAsString().equals("player-id"));
            check(request.get("world_uuid").getAsString().equals("world-id"));
            JsonObject response = new JsonObject();
            response.add("id", request.get("id"));
            if (i == 0) response.addProperty("error", "At capacity.");
            else {
                response.addProperty("answer", request.get("username").getAsString() + " in "
                        + request.get("world_name").getAsString() + "; previous requests: " + i);
                response.add("sources", JsonParser.parseString("[\"https://minecraft.wiki/w/Special:Redirect/revision/100\"]"));
            }
            System.out.println(response);
        }
    }

    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("Bridge boundary assertion failed");
    }
}
