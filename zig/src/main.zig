const std = @import("std");
const process = std.process;
const print = std.debug.print;

// Import modules
const config = @import("config.zig");
const debug = @import("utils/debug.zig");

// Import command handlers
const deps_cmd = @import("commands/deps.zig");
const build_cmd = @import("commands/build.zig");
const artifacts_cmd = @import("commands/artifacts.zig");
const version_cmd = @import("commands/version.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const parsed_config = config.parseArgs(allocator) catch |err| {
        print("Error parsing arguments: {}\n", .{err});
        return;
    };

    // Set global debug mode
    debug.setDebugMode(parsed_config.debug);

    // Execute the appropriate command
    switch (parsed_config.command) {
        .deps => deps_cmd.handleDepsCommand(allocator) catch |err| {
            print("Dependencies command failed: {}\n", .{err});
            process.exit(1);
        },
        .build => build_cmd.handleBuildCommand(allocator, parsed_config.build_options.clean, parsed_config.build_options.sign) catch |err| {
            print("Build command failed: {}\n", .{err});
            process.exit(1);
        },
        .artifacts => artifacts_cmd.handleArtifactsCommand(allocator, parsed_config.artifacts_dir) catch |err| {
            print("Artifacts command failed: {}\n", .{err});
            process.exit(1);
        },
        .version => version_cmd.handleVersionCommand(allocator, parsed_config.version_file) catch |err| {
            print("Version command failed: {}\n", .{err});
            process.exit(1);
        },
        .help => config.printHelp(),
    }
}
