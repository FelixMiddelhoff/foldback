// SPDX-License-Identifier: MIT OR Apache-2.0
using System;
using System.Runtime.InteropServices;

namespace Foldback
{
    // Mirrors `foldback-sys`'s `Config` (crates/foldback-sys/src/lib.rs), field
    // for field, in the same declared order — Rust's `#[repr(C)]` and C#'s
    // `LayoutKind.Sequential` both follow the platform C ABI's layout rules, so
    // matching declaration order here is sufficient, no explicit `Pack`/offsets
    // needed.
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeConfig
    {
        public uint TickRateHz;
        public uint PeerCount;
        public ushort LocalPeerId;
        public uint Retention;

        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
        public byte[] BuildId;

        // Deliberately `IntPtr`, not a marshaled `string` field — risk-plan P1
        // flags IL2CPP AOT as having real restrictions on some automatic
        // marshaling patterns for P/Invoke structs; a raw pointer the managed
        // wrapper allocates/frees itself (see `Utf8Buffer` below) is the
        // conservative choice the risk plan's mitigation calls for.
        public IntPtr RecordToPath;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct NativePendingHash
    {
        public ulong Tick;
        public ulong Hash;
    }

    internal enum NativeStatus : int
    {
        Ok = 0,
        NullPointer = -1,
        InvalidUtf8 = -2,
        Io = -3,
        Panic = -4,
    }

    /// Raw P/Invoke declarations for `foldback-sys`'s C ABI
    /// (foldback-protocol-spec.md §3). Not exposed publicly — see
    /// `FoldbackSession` for the managed, idiomatic wrapper (cookbook recipe 8).
    internal static class FoldbackNative
    {
        // Resolved per-platform by .NET/IL2CPP's own P/Invoke lookup
        // conventions: `foldback_sys.dll` on Windows, `libfoldback_sys.dylib`
        // on macOS, `libfoldback_sys.so` on Linux, as long as the matching
        // native library sits in the Unity project's per-platform Plugins
        // folder (a Unity import-settings concern, not a code one).
        private const string LibName = "foldback_sys";

        [DllImport(LibName)]
        internal static extern IntPtr foldback_session_create(ref NativeConfig config);

        [DllImport(LibName)]
        internal static extern void foldback_session_destroy(IntPtr session);

        [DllImport(LibName)]
        internal static extern NativeStatus foldback_hash_tick(
            IntPtr session,
            ulong tick,
            [In] byte[] state,
            UIntPtr len);

        [DllImport(LibName)]
        internal static extern NativeStatus foldback_record_peer_hash(
            IntPtr session,
            ulong tick,
            ushort peerId,
            ulong hash);

        [DllImport(LibName)]
        internal static extern NativeStatus foldback_hash_entity(
            IntPtr session,
            ulong tick,
            ulong entityId,
            [In] byte[] state,
            UIntPtr len);

        [DllImport(LibName)]
        internal static extern NativeStatus foldback_record_peer_entity_hash(
            IntPtr session,
            ulong tick,
            ushort peerId,
            ulong entityId,
            ulong hash);

        // `fieldName` is a raw pointer, not a marshaled `string` parameter —
        // same conservative reasoning as `NativeConfig.RecordToPath`: this
        // codebase sticks to one verified-under-IL2CPP string-passing
        // pattern (a caller-owned `Utf8Buffer`) rather than introducing a
        // second, differently-marshaled one that hasn't been proven against
        // a real IL2CPP build.
        [DllImport(LibName)]
        internal static extern NativeStatus foldback_hash_field(
            IntPtr session,
            ulong tick,
            ulong entityId,
            IntPtr fieldName,
            [In] byte[] value,
            UIntPtr valueLen);

        [DllImport(LibName)]
        internal static extern NativeStatus foldback_record_peer_field_hash(
            IntPtr session,
            ulong tick,
            ushort peerId,
            ulong entityId,
            IntPtr fieldName,
            ulong hash,
            [In] byte[] value,
            UIntPtr valueLen);

        [DllImport(LibName)]
        internal static extern UIntPtr foldback_pending_hash_count(IntPtr session);

        [DllImport(LibName)]
        internal static extern UIntPtr foldback_take_pending_hashes(
            IntPtr session,
            [Out] NativePendingHash[] outBuf,
            UIntPtr outCapacity);

        [DllImport(LibName)]
        internal static extern NativeStatus foldback_check_divergence(
            IntPtr session,
            out bool outFound,
            out ulong outTick);

        [DllImport(LibName)]
        internal static extern ulong foldback_finish(IntPtr session);

        [DllImport(LibName)]
        internal static extern NativeStatus foldback_finish_recording(IntPtr session);

        [DllImport(LibName)]
        internal static extern uint foldback_tick_rate_hz(IntPtr session);

        [DllImport(LibName)]
        internal static extern uint foldback_peer_count(IntPtr session);

        [DllImport(LibName)]
        internal static extern UIntPtr foldback_last_error([Out] byte[] buf, UIntPtr bufLen);

        /// Reads the calling thread's last error message (see
        /// `foldback_last_error`'s doc comment in `foldback.h`).
        internal static string GetLastError()
        {
            var buf = new byte[1024];
            var len = (int)foldback_last_error(buf, (UIntPtr)buf.Length);
            if (len == 0)
            {
                return "(no error recorded)";
            }
            var truncated = Math.Min(len, buf.Length - 1);
            return System.Text.Encoding.UTF8.GetString(buf, 0, truncated);
        }
    }

    /// Owns a native, NUL-terminated UTF-8 buffer for a single string — used
    /// for `NativeConfig.RecordToPath` instead of automatic string marshaling
    /// (see that field's own comment for why).
    internal sealed class Utf8Buffer : IDisposable
    {
        private IntPtr _ptr;

        public IntPtr Ptr => _ptr;

        public Utf8Buffer(string value)
        {
            var bytes = System.Text.Encoding.UTF8.GetBytes(value + "\0");
            _ptr = Marshal.AllocHGlobal(bytes.Length);
            Marshal.Copy(bytes, 0, _ptr, bytes.Length);
        }

        public void Dispose()
        {
            if (_ptr != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(_ptr);
                _ptr = IntPtr.Zero;
            }
        }
    }
}
