package com.cyhdev.minecraft.ask;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.List;

/** One worker owns the pipe; the deadline thread may close its process. */
final class BridgeClient implements AutoCloseable {
    private final List<String> command;
    private volatile Process process;
    private BufferedReader reader;
    private BufferedWriter writer;
    private long sequence;

    BridgeClient(List<String> command) {
        this.command = List.copyOf(command);
    }

    /** No shell interpolation; questions only enter a bounded JSON stdin frame. */
    synchronized void start() throws IOException {
        if (process != null && process.isAlive()) return;
        ProcessBuilder builder = new ProcessBuilder(command);
        builder.directory(Path.of(command.get(command.size() - 1)).toFile());
        builder.redirectError(ProcessBuilder.Redirect.DISCARD);
        builder.environment().clear();
        builder.environment().put("PATH", "/usr/local/bin:/usr/bin:/bin");
        process = builder.start();
        reader = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8));
        writer = new BufferedWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
    }

    private void ready() throws IOException {
        JsonObject value = parse(readLine(reader));
        if (!value.has("ready") || !value.get("ready").getAsBoolean()
                || value.get("protocol").getAsInt() != 1) throw new IOException("Bridge handshake failed");
    }

    String[] ask(String uuid, String username, String worldUuid, String worldName, String question) throws IOException {
        boolean restart = process == null || !process.isAlive();
        if (restart) {
            start();
            ready();
        }
        long id = ++sequence;
        JsonObject request = new JsonObject();
        request.addProperty("id", id);
        request.addProperty("question", question);
        request.addProperty("player_uuid", uuid);
        request.addProperty("username", username);
        request.addProperty("world_uuid", worldUuid);
        request.addProperty("world_name", worldName);
        writer.write(request.toString());
        writer.newLine();
        writer.flush();
        JsonObject response = parse(readLine(reader));
        if (!response.has("id") || response.get("id").getAsLong() != id) throw new IOException("Bridge request mismatch");
        if (response.has("error")) return new String[] {plain(response.get("error").getAsString()), ""};
        String answer = plain(response.get("answer").getAsString());
        if (answer.isBlank()) throw new IOException("Empty answer");
        String source = "";
        if (response.has("sources") && !response.getAsJsonArray("sources").isEmpty()) {
            source = response.getAsJsonArray("sources").get(0).getAsString();
            if (!source.matches("https://minecraft\\.wiki/w/Special:Redirect/revision/[0-9]+")) {
                throw new IOException("Invalid source URL");
            }
        }
        return new String[] {answer, source};
    }

    private static JsonObject parse(String line) throws IOException {
        try { return JsonParser.parseString(line).getAsJsonObject(); }
        catch (RuntimeException exception) { throw new IOException("Malformed bridge response", exception); }
    }

    static String readLine(BufferedReader reader) throws IOException {
        StringBuilder line = new StringBuilder();
        for (int i = 0; i < 4096; i++) {
            int value = reader.read();
            if (value == -1) throw new IOException("Bridge closed");
            if (value == '\n') return line.toString();
            line.append((char) value);
        }
        throw new IOException("Bridge response exceeds 4096 characters");
    }

    static String plain(String text) {
        StringBuilder result = new StringBuilder();
        text.codePoints().filter(c -> (!Character.isISOControl(c) || Character.isWhitespace(c))
                && c != '§' && Character.getType(c) != Character.FORMAT)
                .forEach(result::appendCodePoint);
        String normalized = result.toString().replaceAll("\\s+", " ").trim();
        String[] words = normalized.split(" ");
        normalized = String.join(" ", java.util.Arrays.copyOf(words, Math.min(words.length, 60)));
        if (normalized.codePointCount(0, normalized.length()) > 360) {
            normalized = normalized.substring(0, normalized.offsetByCodePoints(0, 359)) + "…";
        }
        return normalized;
    }

    @Override
    public synchronized void close() {
        Process active = process;
        if (active == null) return;
        // Only this plugin's child tree is terminated; no server process or shared Codex daemon.
        active.descendants().forEach(ProcessHandle::destroyForcibly);
        active.destroyForcibly();
        process = null;
    }
}
