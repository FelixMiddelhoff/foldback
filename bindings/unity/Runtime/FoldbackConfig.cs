// SPDX-License-Identifier: MIT OR Apache-2.0
namespace Foldback
{
    /// Configuration for a new <see cref="FoldbackSession"/> — matches
    /// `foldback-core::session::SessionBuilder`'s own fields (cookbook
    /// recipes 1-2, 8).
    public struct FoldbackConfig
    {
        public uint TickRateHz;
        public uint PeerCount;

        /// Which peer this local session *is*, for attributing its own
        /// `HashTick` calls in the cross-peer comparison table.
        public ushort LocalPeerId;

        /// Ticks of ring-buffer retention. 0 (the default for an
        /// unset struct) means "use the library default" (600 ticks — 10s
        /// at 60Hz).
        public uint Retention;

        /// 16 bytes identifying the game build, shown in the UI. Null or
        /// any length other than 16 is rejected by <see cref="FoldbackSession"/>'s
        /// constructor — pass null to leave it all zero.
        public byte[] BuildId;

        /// If set, records every frame to a `.foldback` file at this path as
        /// it happens. Null means don't record to a file.
        public string RecordToPath;
    }
}
