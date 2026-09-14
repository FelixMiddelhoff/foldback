// SPDX-License-Identifier: MIT OR Apache-2.0
using System;

namespace Foldback
{
    /// Marks a field or property as tracked for reflective hashing
    /// (foldback-reflective-hashing.md §2.2) — the C#-native mirror of
    /// the Rust derive's `#[foldback(hash)]`/`#[foldback(reflect)]`.
    /// <see cref="FoldbackReflection.HashReflected"/> only walks members
    /// carrying this attribute on the root object's own type; an
    /// unmarked member is invisible to it, same as an unmarked field is
    /// invisible to the manual `HashField` API. Once a tagged member is
    /// reached, everything reachable beneath it is walked without
    /// requiring its own type to be separately tagged — the opt-in
    /// boundary is per top-level member, matching the Bevy and Unreal
    /// bindings' own reflective walkers.
    [AttributeUsage(AttributeTargets.Field | AttributeTargets.Property, AllowMultiple = false)]
    public sealed class FoldbackHashAttribute : Attribute
    {
    }
}
