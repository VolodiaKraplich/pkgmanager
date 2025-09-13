const std = @import("std");
const print = std.debug.print;

var debug_mode: bool = false;

/// Set the global debug mode
pub fn setDebugMode(enabled: bool) void {
    debug_mode = enabled;
}

/// Get the current debug mode state
pub fn isDebugEnabled() bool {
    return debug_mode;
}

/// Debug print function - only prints if debug mode is enabled
pub fn debugPrint(comptime format: []const u8, args: anytype) void {
    if (debug_mode) {
        print("DEBUG: " ++ format ++ "\n", args);
    }
}
