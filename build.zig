const std = @import("std");

pub fn build(b: *std.Build) void {
    const asm_bins_step = b.step("asm-bins", "assemble bins/*.s into .bin outputs");

    const bins_dir = "bins";
    const link_script = "tests/link.ld";
    const target_triple = "aarch64-freestanding-none";

    const cwd = std.fs.cwd();
    var dir = cwd.openDir(bins_dir, .{ .iterate = true }) catch |err| {
        std.debug.panic("failed to open '{s}': {s}", .{ bins_dir, @errorName(err) });
    };
    defer dir.close();

    var it = dir.iterate();
    while (true) {
        const entry_opt = it.next() catch |err| {
            std.debug.panic("failed to iterate '{s}': {s}", .{ bins_dir, @errorName(err) });
        };
        if (entry_opt == null) break;

        const entry = entry_opt.?;
        if (entry.kind != .file) continue;

        const file_name = entry.name;
        if (file_name.len < 3) continue;
        if (!std.mem.endsWith(u8, file_name, ".s")) continue;

        const stem = file_name[0 .. file_name.len - 2];

        const src_path = b.pathJoin(&.{ bins_dir, file_name });
        const elf_out = b.pathJoin(&.{ bins_dir, b.fmt("{s}.elf", .{stem}) });
        const bin_out = b.pathJoin(&.{ bins_dir, b.fmt("{s}.bin", .{stem}) });

        const cc_cmd = b.addSystemCommand(&.{ "zig", "cc" });
        cc_cmd.addArg("-target");
        cc_cmd.addArg(target_triple);
        cc_cmd.addArg("-nostdlib");
        cc_cmd.addArg(b.fmt("-Wl,-T,{s}", .{link_script}));
        cc_cmd.addArg("-o");
        cc_cmd.addArg(elf_out);
        cc_cmd.addArg(src_path);

        const objcopy_cmd = b.addSystemCommand(&.{ "zig", "objcopy" });
        objcopy_cmd.addArg("-O");
        objcopy_cmd.addArg("binary");
        objcopy_cmd.addArg(elf_out);
        objcopy_cmd.addArg(bin_out);

        // keep the order: compile -> objcopy
        objcopy_cmd.step.dependOn(&cc_cmd.step);

        // running `zig build asm-bins` executes all these
        asm_bins_step.dependOn(&objcopy_cmd.step);
    }
}
