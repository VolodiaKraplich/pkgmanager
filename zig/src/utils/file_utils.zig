const std = @import("std");
const fs = std.fs;
const mem = std.mem;
const ArrayList = std.ArrayList;

/// Copy file from source to destination, preserving permissions
pub fn copyFile(src_path: []const u8, dst_path: []const u8) !void {
    const src_file = try fs.cwd().openFile(src_path, .{});
    defer src_file.close();

    const dst_file = try fs.cwd().createFile(dst_path, .{});
    defer dst_file.close();

    var buffer: [4096]u8 = undefined;
    while (true) {
        const bytes_read = try src_file.read(&buffer);
        if (bytes_read == 0) break;
        try dst_file.writeAll(buffer[0..bytes_read]);
    }

    // Copy permissions
    const src_stat = try src_file.stat();
    try dst_file.chmod(src_stat.mode);
}

/// Find files matching any of the given patterns in the current directory
pub fn findFilesWithPatterns(allocator: std.mem.Allocator, patterns: []const []const u8) !ArrayList([]const u8) {
    var found_files = ArrayList([]const u8).init(allocator);

    var dir = fs.cwd().openDir(".", .{ .iterate = true }) catch return found_files;
    defer dir.close();

    var iterator = dir.iterate();
    while (try iterator.next()) |entry| {
        if (entry.kind != .file) continue;

        for (patterns) |pattern| {
            if (mem.indexOf(u8, entry.name, pattern)) |_| {
                const owned_name = try allocator.dupe(u8, entry.name);
                try found_files.append(owned_name);
                break;
            }
        }
    }

    return found_files;
}

/// Clean up directory by removing files and directories
pub fn cleanBuildDirectory() void {
    var dir = fs.cwd().openDir(".", .{ .iterate = true }) catch return;
    defer dir.close();

    var iterator = dir.iterate();
    while (iterator.next() catch null) |entry| {
        if (entry.kind == .file) {
            if (mem.indexOf(u8, entry.name, ".pkg.tar.")) |_| {
                fs.cwd().deleteFile(entry.name) catch {};
            }
        }
    }

    // Remove directories
    fs.cwd().deleteTree("src") catch {};
    fs.cwd().deleteTree("pkg") catch {};
}
