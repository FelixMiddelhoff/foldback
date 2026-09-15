// SPDX-License-Identifier: MIT OR Apache-2.0
using System;
using System.Collections.Generic;

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

        /// Level 2: hashes one entity's state and records it as this
        /// session's own report for `tick` (cookbook recipe 4).
        /// Recording-only — a no-op beyond the hash call unless this
        /// session was created with <see cref="FoldbackConfig.RecordToPath"/>
        /// set; there's no in-memory Level 2 divergence tracking yet.
        public void HashEntity(ulong tick, ulong entityId, byte[] state)
        {
            ThrowIfDisposed();
            var bytes = state ?? Array.Empty<byte>();
            var status = FoldbackNative.foldback_hash_entity(_handle, tick, entityId, bytes, (UIntPtr)bytes.Length);
            ThrowIfError(status, "HashEntity");
        }

        /// Records an entity hash reported by any peer (including this
        /// session's own, via <see cref="HashEntity"/>).
        public void RecordPeerEntityHash(ulong tick, ushort peerId, ulong entityId, ulong hash)
        {
            ThrowIfDisposed();
            var status = FoldbackNative.foldback_record_peer_entity_hash(_handle, tick, peerId, entityId, hash);
            ThrowIfError(status, "RecordPeerEntityHash");
        }

        /// Level 3: hashes one field's value and records it as this
        /// session's own report for `tick`, keeping the raw value bytes too
        /// (cookbook recipe 5) — this is what lets the UI show
        /// "3.14159 vs 3.14158" instead of just two unequal hashes.
        /// Recording-only, same caveat as <see cref="HashEntity"/>.
        public void HashField(ulong tick, ulong entityId, string fieldName, byte[] value)
        {
            ThrowIfDisposed();
            var bytes = value ?? Array.Empty<byte>();
            using var name = new Utf8Buffer(fieldName ?? throw new ArgumentNullException(nameof(fieldName)));
            var status = FoldbackNative.foldback_hash_field(
                _handle, tick, entityId, name.Ptr, bytes, (UIntPtr)bytes.Length);
            ThrowIfError(status, "HashField");
        }

        /// Records a field hash reported by any peer.
        public void RecordPeerFieldHash(ulong tick, ushort peerId, ulong entityId, string fieldName, ulong hash, byte[] value)
        {
            ThrowIfDisposed();
            var bytes = value ?? Array.Empty<byte>();
            using var name = new Utf8Buffer(fieldName ?? throw new ArgumentNullException(nameof(fieldName)));
            var status = FoldbackNative.foldback_record_peer_field_hash(
                _handle, tick, peerId, entityId, name.Ptr, hash, bytes, (UIntPtr)bytes.Length);
            ThrowIfError(status, "RecordPeerFieldHash");
        }

        /// Records `typeName`'s current tagged field set as a `Metadata`
        /// frame — the schema-drift detection mechanism
        /// (foldback-reflective-hashing.md §7). Called by
        /// <see cref="FoldbackReflection.HashReflected"/> once per tracked
        /// type it walks; a no-op past the first call for a given
        /// `typeName` this session, same as the native
        /// `Session::record_schema` it wraps.
        public void RecordSchema(string typeName, IReadOnlyList<string> trackedFields)
        {
            ThrowIfDisposed();
            using var name = new Utf8Buffer(typeName ?? throw new ArgumentNullException(nameof(typeName)));
            var fields = trackedFields ?? Array.Empty<string>();
            var buffers = new Utf8Buffer[fields.Count];
            var ptrs = new IntPtr[fields.Count];
            try
            {
                for (var i = 0; i < fields.Count; i++)
                {
                    buffers[i] = new Utf8Buffer(fields[i]);
                    ptrs[i] = buffers[i].Ptr;
                }
                var status = FoldbackNative.foldback_record_schema(_handle, name.Ptr, ptrs, (UIntPtr)ptrs.Length);
                ThrowIfError(status, "RecordSchema");
            }
            finally
            {
                foreach (var buffer in buffers)
                {
                    buffer?.Dispose();
                }
            }
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
