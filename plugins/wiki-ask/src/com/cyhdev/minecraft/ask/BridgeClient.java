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
import java.util.function.Consumer;

/** One worker owns the pipe; the deadline thread may close its process. */
final class BridgeClient implements AutoCloseable {
    private final List<String> command;
    private volatile Process process;
    private boolean stopped;
    private BufferedReader reader;
    private BufferedWriter writer;
    private long sequence;
    private final Consumer<String> progress;

    BridgeClient(List<String> command) {
        this(command, phase -> {});
    }

    BridgeClient(List<String> command, Consumer<String> progress) {
        this.command = List.copyOf(command);
        this.progress = progress;
    }

    /** No shell interpolation; questions only enter a bounded JSON stdin frame. */
    synchronized void start() throws IOException {
        if (stopped) throw new IOException("Bridge is stopped");
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
        if (value.has("ready") && !value.get("ready").getAsBoolean() && value.has("diagnostic")) {
            throw new IOException("Bridge startup failed: " + plain(value.get("diagnostic").getAsString()));
        }
        if (!value.has("ready") || !value.get("ready").getAsBoolean()
                || value.get("protocol").getAsInt() != 2) throw new IOException("Bridge handshake failed; install matching jar and companion");
    }

    /** Load the corpus and establish IPC during plugin startup, away from the tick thread. */
    void prepare() throws IOException {
        if (process == null || !process.isAlive()) {
            progress.accept("starting");
            start();
            ready();
        }
    }

    String[] ask(String uuid, String username, String worldUuid, String worldName, String question) throws IOException {
        return request(uuid, username, worldUuid, worldName, question, "ask");
    }

    /** Control operations are explicit protocol fields, never prompts interpreted by the model. */
    String[] request(String uuid, String username, String worldUuid, String worldName, String question, String action) throws IOException {
        if (!List.of("ask", "clear", "compact").contains(action)) throw new IOException("Unknown operation");
        prepare();
        long id = ++sequence;
        JsonObject request = new JsonObject();
        request.addProperty("id", id);
        request.addProperty("question", question);
        request.addProperty("action", action);
        request.addProperty("player_uuid", uuid);
        request.addProperty("username", username);
        request.addProperty("world_uuid", worldUuid);
        request.addProperty("world_name", worldName);
        writer.write(request.toString());
        writer.newLine();
        writer.flush();
        String operation = action.equals("ask") ? "connecting" : action.equals("compact") ? "compacting" : "clearing";
        JsonObject response = null;
        for (int frames = 0; frames <= 32; frames++) {
            response = parse(readLine(reader));
            if (!response.has("id") || response.get("id").getAsLong() != id) throw new IOException("Bridge request mismatch");
            if (!response.has("status")) break;
            if (frames == 32 || response.size() != 2) throw new IOException("Invalid bridge progress stream");
            String phase = response.get("status").getAsString();
            RequestStatus.description(phase);
            progress.accept(phase);
            if (!phase.equals("resetting")) operation = phase;
        }
        if (response == null) throw new IOException("Missing bridge response");
        if (response.has("error_code")) {
            String message = RequestStatus.failure(response.get("error_code").getAsString(), operation);
            if (response.has("fatal") && response.get("fatal").getAsBoolean()) close();
            return new String[] {message, ""};
        }
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

    static String action(String[] args) {
        if (args.length == 1 && (args[0].equalsIgnoreCase("clear") || args[0].equalsIgnoreCase("compact"))) {
            return args[0].toLowerCase(java.util.Locale.ROOT);
        }
        return "ask";
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

    /** Prevent a startup racing plugin disable from leaving an orphaned child. */
    synchronized void shutdown() {
        stopped = true;
        close();
    }
}
