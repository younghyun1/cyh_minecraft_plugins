package com.cyhdev.minecraft;

import com.google.gson.Strictness;
import com.google.gson.stream.JsonReader;
import com.google.gson.stream.JsonToken;
import java.io.IOException;
import java.io.StringReader;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.FileVisitOption;
import java.nio.file.FileVisitResult;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.HashSet;
import java.util.EnumSet;
import java.util.LinkedHashMap;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;

/** Bounded worker-thread inspection of packs that cannot replace generation resources. */
final class PredictionPacks {
    static final int MAX_PACKS = 8;
    private static final int MAX_ENTRIES = 256;
    private static final int MAX_DEPTH = 10;
    private static final int MAX_FILE_BYTES = 64 * 1024;
    private static final int MAX_PACK_BYTES = 1024 * 1024;
    private static final Set<String> BUILTIN = Set.of("vanilla", "paper");
    private static final Set<String> PACK_FIELDS = Set.of("description", "pack_format", "min_format", "max_format");

    private PredictionPacks() {}

    record Selection(Path directory, List<String> names) {}

    static boolean supportedName(String name) {
        if (BUILTIN.contains(name)) return true;
        if (!name.startsWith("file/")) return false;
        String file = name.substring(5);
        return file.matches("[a-zA-Z0-9_.-]{1,96}") && !file.equals(".") && !file.equals("..") && !file.endsWith(".zip");
    }

    /** Call between main-thread profile captures, then repeat before publishing the profile. */
    static String fingerprint(Selection selection, long expires) throws IOException {
        if (selection.names().isEmpty() || selection.names().size() > MAX_PACKS
                || !selection.names().contains("vanilla") || new HashSet<>(selection.names()).size() != selection.names().size()) {
            throw unsupported();
        }
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            for (String name : selection.names().stream().sorted().toList()) {
                checkDeadline(expires);
                if (!supportedName(name)) throw unsupported();
                update(digest, name);
                if (BUILTIN.contains(name)) continue;
                attributes(selection.directory(), true);
                inspect(selection.directory().resolve(name.substring(5)), digest, expires);
            }
            return HexFormat.of().formatHex(digest.digest());
        } catch (NoSuchAlgorithmException error) { throw new IOException("Pack fingerprint unavailable", error); }
    }

    private static void inspect(Path root, MessageDigest digest, long expires) throws IOException {
        BasicFileAttributes rootBefore = attributes(root, true);
        var observed = new LinkedHashMap<Path, BasicFileAttributes>();
        Files.walkFileTree(root, EnumSet.noneOf(FileVisitOption.class), MAX_DEPTH, new SimpleFileVisitor<>() {
            private void record(Path path, BasicFileAttributes attributes) throws IOException {
                checkDeadline(expires);
                if (observed.size() >= MAX_ENTRIES || attributes.isSymbolicLink()
                        || (!attributes.isDirectory() && !attributes.isRegularFile())) throw unsupported();
                observed.put(path, attributes);
                if (path.equals(root)) return;
                checkAncestors(path.getParent(), root, observed);
                String relative = root.relativize(path).toString().replace('\\', '/');
                if (relative.length() > 512 || root.relativize(path).getNameCount() >= MAX_DEPTH
                        || !allowedPath(relative, attributes.isDirectory())) throw unsupported();
            }
            @Override public FileVisitResult preVisitDirectory(Path path, BasicFileAttributes attributes) throws IOException {
                record(path, attributes);
                return FileVisitResult.CONTINUE;
            }
            @Override public FileVisitResult visitFile(Path path, BasicFileAttributes attributes) throws IOException {
                record(path, attributes);
                return FileVisitResult.CONTINUE;
            }
        });
        boolean metadata = false;
        int totalBytes = 0;
        for (Path path : observed.keySet().stream().sorted().toList()) {
            checkDeadline(expires);
            var before = observed.get(path);
            if (path.equals(root)) continue;
            checkAncestors(path.getParent(), root, observed);
            String relative = root.relativize(path).toString().replace('\\', '/');
            update(digest, before.isDirectory() ? "directory" : "file");
            update(digest, relative);
            if (before.isDirectory()) continue;
            int limit = relative.equals("pack.mcmeta") ? 8192 : MAX_FILE_BYTES;
            if (before.size() > limit || totalBytes + before.size() > MAX_PACK_BYTES) throw unsupported();
            byte[] bytes;
            try (var input = Files.newInputStream(path, LinkOption.NOFOLLOW_LINKS)) { bytes = input.readNBytes(limit + 1); }
            checkDeadline(expires);
            checkAncestors(path.getParent(), root, observed);
            if (bytes.length != before.size() || !unchanged(before, attributes(path, false))) throw unsupported();
            totalBytes += bytes.length;
            if (relative.equals("pack.mcmeta")) { validateMetadata(bytes); metadata = true; }
            update(digest, bytes);
        }
        for (var entry : observed.entrySet()) {
            checkDeadline(expires);
            if (!unchanged(entry.getValue(), attributes(entry.getKey(), entry.getValue().isDirectory()))) throw unsupported();
        }
        if (!metadata || !unchanged(rootBefore, attributes(root, true))) throw unsupported();
    }

    private static boolean allowedPath(String relative, boolean directory) {
        if (relative.equals("pack.mcmeta")) return !directory;
        String[] parts = relative.split("/");
        if (!parts[0].equals("data")) return false;
        if (parts.length == 1) return directory;
        for (int i = 1; i < parts.length; i++) if (!parts[i].matches("[a-z0-9_.-]+")) return false;
        if (parts.length == 2) return directory;
        int firstResource;
        if (parts[2].equals("function")) firstResource = 3;
        else if (parts[2].equals("tags")) {
            if (parts.length == 3) return directory;
            if (!parts[3].equals("function")) return false;
            firstResource = 4;
        } else return false;
        if (parts.length <= firstResource) return directory;
        return directory || relative.endsWith(firstResource == 3 ? ".mcfunction" : ".json");
    }

    /** No filters, overlays, feature flags, duplicate fields, or unrecognized metadata. */
    private static void validateMetadata(byte[] bytes) throws IOException {
        try (var reader = new JsonReader(new StringReader(new String(bytes, StandardCharsets.UTF_8)))) {
            reader.setStrictness(Strictness.STRICT);
            reader.beginObject();
            if (!reader.hasNext() || !reader.nextName().equals("pack")) throw unsupported();
            reader.beginObject();
            Set<String> fields = new HashSet<>();
            while (reader.hasNext()) {
                String field = reader.nextName();
                if (!PACK_FIELDS.contains(field) || !fields.add(field)) throw unsupported();
                if (field.equals("description")) {
                    if (reader.peek() != JsonToken.STRING) throw unsupported();
                    reader.nextString();
                } else {
                    if (reader.peek() != JsonToken.NUMBER || !reader.nextString().matches("[0-9]{1,6}")) throw unsupported();
                }
            }
            reader.endObject();
            if (!fields.contains("description") || !(fields.contains("pack_format")
                    || fields.containsAll(Set.of("min_format", "max_format"))) || reader.hasNext()) throw unsupported();
            reader.endObject();
            if (reader.peek() != JsonToken.END_DOCUMENT) throw unsupported();
        } catch (IllegalArgumentException | IllegalStateException error) { throw unsupported(); }
    }

    private static BasicFileAttributes attributes(Path path, boolean directory) throws IOException {
        var result = Files.readAttributes(path, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
        if (result.isSymbolicLink() || (directory ? !result.isDirectory() : !result.isRegularFile())) throw unsupported();
        return result;
    }

    private static boolean unchanged(BasicFileAttributes before, BasicFileAttributes after) {
        return before.size() == after.size() && before.lastModifiedTime().equals(after.lastModifiedTime())
            && java.util.Objects.equals(before.fileKey(), after.fileKey());
    }

    private static void checkAncestors(Path path, Path root, java.util.Map<Path, BasicFileAttributes> observed) throws IOException {
        for (Path current = path; current != null && current.startsWith(root); current = current.getParent()) {
            var before = observed.get(current);
            if (before == null || !unchanged(before, attributes(current, true))) throw unsupported();
        }
    }

    private static void update(MessageDigest digest, String value) {
        update(digest, value.getBytes(StandardCharsets.UTF_8));
    }

    private static void update(MessageDigest digest, byte[] value) {
        digest.update(ByteBuffer.allocate(Integer.BYTES).putInt(value.length).array());
        digest.update(value);
    }

    private static void checkDeadline(long expires) throws IOException {
        if (Thread.currentThread().isInterrupted() || System.nanoTime() >= expires) throw new IOException("Datapack inspection expired");
    }

    private static IOException unsupported() { return new IOException("Unsupported prediction datapack"); }
}
