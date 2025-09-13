const std = @import("std");
const mem = std.mem;
const print = std.debug.print;
const pkgbuild = @import("../pkgbuild.zig");
const process_utils = @import("../utils/process_utils.zig");

/// Handle the dependencies installation command
pub fn handleDepsCommand(allocator: std.mem.Allocator) !void {
    print("Installing PKGBUILD dependencies...\n", .{});

    var info = pkgbuild.parsePkgbuild(allocator, "PKGBUILD") catch |err| {
        print("Error parsing PKGBUILD: {}\n", .{err});
        return;
    };
    defer info.deinit(allocator);

    var all_deps = try info.getAllDependencies(allocator);
    defer all_deps.deinit(allocator);

    if (all_deps.items.len == 0) {
        print("No dependencies found in PKGBUILD.\n", .{});
        return;
    }

    print("Found dependencies: ", .{});
    for (all_deps.items, 0..) |dep, i| {
        if (i > 0) print(", ", .{});
        print("{s}", .{dep});
    }
    print("\n", .{});

    // Handle rust/rustup conflict and filter dependencies
    var filtered_deps = try filterDependencies(allocator, all_deps.items);
    defer filtered_deps.deinit(allocator);

    if (filtered_deps.items.len == 0) {
        print("All dependencies are already satisfied.\n", .{});
        return;
    }

    try installDependencies(allocator, filtered_deps.items);
    print("Dependencies installation completed!\n", .{});
}

/// Filter dependencies to handle special cases like rust/rustup conflicts
fn filterDependencies(allocator: std.mem.Allocator, deps: []const []const u8) !std.ArrayListUnmanaged([]const u8) {
    var filtered_deps = std.ArrayListUnmanaged([]const u8){};

    var has_rust = false;
    var has_rustup = false;

    for (deps) |dep| {
        if (mem.eql(u8, dep, "rust")) {
            has_rust = true;
        } else if (mem.eql(u8, dep, "rustup")) {
            has_rustup = true;
        } else {
            try filtered_deps.append(allocator, dep);
        }
    }

    if (has_rust or has_rustup) {
        try handleRustDependency(allocator, &filtered_deps);
    }

    return filtered_deps;
}

/// Handle rust/rustup dependency conflicts
fn handleRustDependency(allocator: std.mem.Allocator, filtered_deps: *std.ArrayListUnmanaged([]const u8)) !void {
    // Check if rustup is available
    const rustup_check = process_utils.runCommand(allocator, &.{ "which", "rustup" });
    if (rustup_check) {
        print("rustup is already available, skipping rust package\n", .{});
        // Remove cargo if it exists
        var i: usize = 0;
        while (i < filtered_deps.items.len) {
            if (mem.eql(u8, filtered_deps.items[i], "cargo")) {
                _ = filtered_deps.swapRemove(i);
            } else {
                i += 1;
            }
        }
    } else |_| {
        print("Installing rustup for Rust toolchain...\n", .{});
        try filtered_deps.append(allocator, "rustup");
    }
}

/// Install dependencies using paru or fallback to pacman
fn installDependencies(allocator: std.mem.Allocator, deps: []const []const u8) !void {
    // Try paru first
    var paru_args = std.ArrayListUnmanaged([]const u8){};
    defer paru_args.deinit(allocator);

    try paru_args.appendSlice(allocator, &.{ "paru", "-S", "--noconfirm", "--needed", "--asdeps" });
    try paru_args.appendSlice(allocator, deps);

    if (process_utils.runCommand(allocator, paru_args.items)) {
        return;
    } else |_| {
        print("Paru failed, trying with sudo pacman...\n", .{});
        try installWithPacman(allocator, deps);
    }
}

/// Fallback installation using pacman
fn installWithPacman(allocator: std.mem.Allocator, deps: []const []const u8) !void {
    var pacman_args = std.ArrayListUnmanaged([]const u8){};
    defer pacman_args.deinit(allocator);

    try pacman_args.appendSlice(allocator, &.{ "sudo", "pacman", "-S", "--noconfirm", "--needed", "--asdeps" });
    try pacman_args.appendSlice(allocator, deps);

    process_utils.runCommand(allocator, pacman_args.items) catch |err| {
        print("Warning: Some dependencies might not be available: {}\n", .{err});
    };
    print("Dependencies installation attempted with pacman!\n", .{});
}
