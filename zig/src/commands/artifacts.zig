const std = @import("std");
const fs = std.fs;
const mem = std.mem;
const print = std.debug.print;
const file_utils = @import("../utils/file_utils.zig");
const errors = @import("../errors.zig");

/// Handle the artifacts collection command
pub fn handleArtifactsCommand(allocator: std.mem.Allocator, artifacts_dir: []const u8) !void {
    print("Collecting build artifacts into directory: {s}\n", .{artifacts_dir});

    // Create artifacts directory
    fs.cwd().makeDir(artifacts_dir) catch |err| switch (err) {
        error.PathAlreadyExists => {},
        else => return err,
    };

    const patterns = [_][]const u8{ ".pkg.tar.", ".log", "PKGBUILD", ".SRCINFO" };
    var found_packages = false;

    var dir = fs.cwd().openDir(".", .{ .iterate = true }) catch return;
    defer dir.close();

    var iterator = dir.iterate();
    while (try iterator.next()) |entry| {
        if (entry.kind != .file) continue;

        for (patterns) |pattern| {
            if (mem.indexOf(u8, entry.name, pattern)) |_| {
                try processArtifact(allocator, entry.name, artifacts_dir, pattern, &found_packages);
                break;
            }
        }
    }

    if (!found_packages) {
        print("Error: No package files (*.pkg.tar.*) were found to collect.\n", .{});
        return errors.BuilderError.NoArtifactsFound;
    }

    print("Artifacts collected successfully.\n", .{});
}

/// Process a single artifact file
fn processArtifact(allocator: std.mem.Allocator, filename: []const u8, artifacts_dir: []const u8, pattern: []const u8, found_packages: *bool) !void {
    const dest_path = try std.fmt.allocPrint(allocator, "{s}/{s}", .{ artifacts_dir, filename });
    defer allocator.free(dest_path);

    if (mem.eql(u8, filename, "PKGBUILD")) {
        // Copy PKGBUILD instead of moving it
        file_utils.copyFile(filename, dest_path) catch |err| {
            print("Warning: could not copy artifact {s}: {}\n", .{ filename, err });
            return;
        };
        print("  Copied: {s}\n", .{dest_path});
    } else {
        // Move other files
        fs.cwd().rename(filename, dest_path) catch |err| {
            print("Warning: could not move artifact {s}: {}\n", .{ filename, err });
            return;
        };
        print("  Collected: {s}\n", .{dest_path});

        if (mem.indexOf(u8, pattern, ".pkg.tar.")) |_| {
            found_packages.* = true;
        }
    }
}
