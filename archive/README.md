# Historical backup source

These components are retained to keep the custom Minecraft source together. They are excluded from the maintained Rust workspace and are not recommended operational backup commands.

`mc` is an early tar/gzip experiment that collects directory entries and file contents in memory and writes beneath its source directory. `mc-backup` builds an entire gzip archive in memory. Both preserve their existing code and lockfiles for provenance; no invocation or backup was performed during consolidation.

`streaming-backup/minecraft.rs` is a newer tar/zstd component using bounded streaming buffers and a blocking worker. It depends on the original application's configuration type and is a reference module rather than a standalone crate. A supported backup CLI needs explicit source/destination arguments, consistency policy, recovery tests and storage integration before replacing the existing operational process.
