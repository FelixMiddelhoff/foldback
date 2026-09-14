// SPDX-License-Identifier: MIT OR Apache-2.0
using System;

namespace Foldback
{
    /// One hash produced locally, ready to send to peers over the game's own
    /// netcode channel — mirrors `foldback_core::session::PendingHash`.
    public readonly struct PendingHash
    {
        public readonly ulong Tick;
        public readonly ulong Hash;

        internal PendingHash(ulong tick, ulong hash)
        {
            Tick = tick;
            Hash = hash;
        }
    }

    /// Thrown when a `foldback-sys` call fails — the message is whatever
    /// `foldback_last_error` reported.
    public sealed class FoldbackException : Exception
    {
        internal FoldbackException(string message) : base(message) { }
    }

    /// The Unity-side integration surface (cookbook recipe 8) — a thin,
    /// idiomatic C# wrapper over `foldback-sys`'s C ABI. Level 1 (per-tick)
    /// only, matching what `foldback-sys` itself currently exposes across the
    /// FFI boundary; Level 2/3 (entity/field) hashing here is a deliberate
    /// follow-up once `foldback-sys` grows that surface.
    public sealed class FoldbackSession : IDisposable
    {
        private IntPtr _handle;
        private Utf8Buffer _recordToPathBuf;
        private bool _disposed;

        public FoldbackSession(FoldbackConfig config)
        {
            var buildId = config.BuildId ?? new byte[16];
            if (buildId.Length != 16)
            {
                throw new ArgumentException("BuildId must be exactly 16 bytes.", nameof(config));
            }

            _recordToPathBuf = config.RecordToPath != null ? new Utf8Buffer(config.RecordToPath) : null;

            var native = new NativeConfig
            {
                TickRateHz = config.TickRateHz,
                PeerCount = config.PeerCount,
                LocalPeerId = config.LocalPeerId,
                Retention = config.Retention,
                BuildId = buildId,
                RecordToPath = _recordToPathBuf?.Ptr ?? IntPtr.Zero,
            };

            _handle = FoldbackNative.foldback_session_create(ref native);
            if (_handle == IntPtr.Zero)
            {
                _recordToPathBuf?.Dispose();
                _recordToPathBuf = null;
                throw new FoldbackException(
                    "could not create Foldback session: " + FoldbackNative.GetLastError());
            }
        }

        /// Hashes `state` and records it as this session's own report for
        /// `tick` — each `FixedUpdate`, per cookbook recipe 8.
        public void HashTick(ulong tick, byte[] state)
        {
            ThrowIfDisposed();
            var bytes = state ?? Array.Empty<byte>();
            var status = FoldbackNative.foldback_hash_tick(_handle, tick, bytes, (UIntPtr)bytes.Length);
            ThrowIfError(status, "HashTick");
        }

        /// Records a hash reported by any peer (received over the game's own
        /// netcode, alongside the peer's own <see cref="TakePendingHashes"/> —
        /// cookbook recipe 1).
        public void RecordPeerHash(ulong tick, ushort peerId, ulong hash)
        {
            ThrowIfDisposed();
            var status = FoldbackNative.foldback_record_peer_hash(_handle, tick, peerId, hash);
            ThrowIfError(status, "RecordPeerHash");
        }

        /// Drains and returns hashes produced locally since the last call —
        /// send these to peers over the game's own netcode channel.
        public PendingHash[] TakePendingHashes()
        {
            ThrowIfDisposed();
            var count = (int)FoldbackNative.foldback_pending_hash_count(_handle);
            if (count == 0)
            {
                return Array.Empty<PendingHash>();
            }

            var native = new NativePendingHash[count];
            var written = (int)FoldbackNative.foldback_take_pending_hashes(_handle, native, (UIntPtr)count);
            var result = new PendingHash[written];
            for (var i = 0; i < written; i++)
            {
                result[i] = new PendingHash(native[i].Tick, native[i].Hash);
            }
            return result;
        }

        /// Returns true (with `tick` set) if a new cross-peer divergence was
        /// found since the last call — false otherwise.
        public bool CheckDivergence(out ulong tick)
        {
            ThrowIfDisposed();
            var status = FoldbackNative.foldback_check_divergence(_handle, out var found, out tick);
            ThrowIfError(status, "CheckDivergence");
            return found;
        }

        /// A single combined hash over every tick this session has locally
        /// hashed — equal between two independent runs iff every tick hashed
        /// identically (the CI-gate pattern).
        public ulong Finish()
        {
            ThrowIfDisposed();
            return FoldbackNative.foldback_finish(_handle);
        }

        /// Writes the closing frame if recording to a file — call before
        /// disposing when `RecordToPath` was set.
        public void FinishRecording()
        {
            ThrowIfDisposed();
            var status = FoldbackNative.foldback_finish_recording(_handle);
            ThrowIfError(status, "FinishRecording");
        }

        public uint TickRateHz
        {
            get
            {
                ThrowIfDisposed();
                return FoldbackNative.foldback_tick_rate_hz(_handle);
            }
        }

        public uint PeerCount
        {
            get
            {
                ThrowIfDisposed();
                return FoldbackNative.foldback_peer_count(_handle);
            }
        }

        public void Dispose()
        {
            if (_disposed)
            {
                return;
            }
            if (_handle != IntPtr.Zero)
            {
                FoldbackNative.foldback_session_destroy(_handle);
                _handle = IntPtr.Zero;
            }
            _recordToPathBuf?.Dispose();
            _recordToPathBuf = null;
            _disposed = true;
        }

        private void ThrowIfDisposed()
        {
            if (_disposed)
            {
                throw new ObjectDisposedException(nameof(FoldbackSession));
            }
        }

        private static void ThrowIfError(NativeStatus status, string call)
        {
            if (status != NativeStatus.Ok)
            {
                throw new FoldbackException(
                    $"{call} failed ({status}): {FoldbackNative.GetLastError()}");
            }
        }
    }
}
