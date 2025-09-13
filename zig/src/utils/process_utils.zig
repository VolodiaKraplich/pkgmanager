const std = @import("std");
const process = std.process;
const print = std.debug.print;
const debug = @import("debug.zig");
const errors = @import("../errors.zig");

/// Run command and stream output
pub fn runCommand(allocator: std.mem.Allocator, cmd_args: []const []const u8) !void {
    const cmd_str = try std.mem.join(allocator, " ", cmd_args);
    defer allocator.free(cmd_str);

    debug.debugPrint("Running command: {s}", .{cmd_str});
    if (!debug.isDebugEnabled()) {
        print("+ Running command: {s}\n", .{cmd_str});
    }

    var child = process.Child.init(cmd_args, allocator);
    child.stdout_behavior = .Inherit;
    child.stderr_behavior = .Inherit;

    const term = try child.spawnAndWait();
    switch (term) {
        .Exited => |code| {
            if (code != 0) {
                return errors.BuilderError.CommandFailed;
            }
        },
        else => return errors.BuilderError.CommandFailed,
    }
}

/// Run command with custom environment
pub fn runCommandWithEnv(allocator: std.mem.Allocator, cmd_args: []const []const u8, env_vars: []const [2][]const u8) !void {
    const cmd_str = try std.mem.join(allocator, " ", cmd_args);
    defer allocator.free(cmd_str);

    debug.debugPrint("Running command with env: {s}", .{cmd_str});
    if (!debug.isDebugEnabled()) {
        print("+ Running command: {s}\n", .{cmd_str});
    }

    var child = process.Child.init(cmd_args, allocator);
    child.stdout_behavior = .Inherit;
    child.stderr_behavior = .Inherit;

    // Set up environment
    var env_map = try process.getEnvMap(allocator);
    defer env_map.deinit();

    for (env_vars) |env_var| {
        try env_map.put(env_var[0], env_var[1]);
    }

    child.env_map = &env_map;

    const term = try child.spawnAndWait();
    switch (term) {
        .Exited => |code| {
            if (code != 0) {
                print("Command failed with exit code: {d}\n", .{code});
                return errors.BuilderError.CommandFailed;
            }
        },
        else => {
            print("Command failed\n", .{});
            return errors.BuilderError.CommandFailed;
        },
    }
}
