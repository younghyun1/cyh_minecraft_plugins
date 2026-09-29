package com.cyhdev.minecraft;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.Arrays;

/** Header-only coverage checks never decode, load, save, or generate a chunk. */
final class SavedChunkCoverage {
    private static final int HEADER_BYTES = 8192;

    private SavedChunkCoverage() {}

    /** An allocated entry might be a protochunk, so only a stable absent entry proves absence. */
    static boolean absent(Path regionDirectory, int chunkX, int chunkZ) throws IOException {
        Path path = regionDirectory.resolve("r." + Math.floorDiv(chunkX, 32) + "." + Math.floorDiv(chunkZ, 32) + ".mca");
        try {
            BasicFileAttributes before = Files.readAttributes(path, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
            if (!before.isRegularFile()) throw new IOException("Region is not a regular file");
            try (FileChannel channel = FileChannel.open(path, StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS)) {
                ByteBuffer first = header(channel);
                int index = Math.floorMod(chunkX, 32) + Math.floorMod(chunkZ, 32) * 32;
                if (first.getInt(index * 4) != 0) return false;
                ByteBuffer second = header(channel);
                BasicFileAttributes after = Files.readAttributes(path, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
                // File replacement or any simultaneous save makes absence uncertain.
                return after.isRegularFile() && before.fileKey() != null && before.fileKey().equals(after.fileKey())
                    && before.size() == after.size() && before.lastModifiedTime().equals(after.lastModifiedTime())
                    && Arrays.equals(first.array(), second.array());
            }
        } catch (NoSuchFileException missing) {
            // Only a positively identified directory and a still-absent child count as missing data.
            BasicFileAttributes directory = Files.readAttributes(regionDirectory, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
            return directory.isDirectory() && Files.notExists(path, LinkOption.NOFOLLOW_LINKS);
        }
    }

    private static ByteBuffer header(FileChannel channel) throws IOException {
        ByteBuffer buffer = ByteBuffer.allocate(HEADER_BYTES);
        while (buffer.hasRemaining()) {
            if (channel.read(buffer, buffer.position()) <= 0) throw new IOException("Incomplete region header");
        }
        return buffer.flip();
    }
}
