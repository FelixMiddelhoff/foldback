// SPDX-License-Identifier: MIT OR Apache-2.0
using System;
using System.Collections;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Linq.Expressions;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text;

namespace Foldback
{
    /// Unity/C# reflective hashing (foldback-reflective-hashing.md §2.2) —
    /// walks <see cref="FoldbackHashAttribute"/>-tagged fields/properties
    /// via <see cref="System.Reflection"/> and records each leaf through
    /// <see cref="FoldbackSession.HashField"/>, so a game gets Level 3
    /// bisection without hand-writing <c>HashField</c> calls per field.
    ///
    /// <para><b>Opt-in survives the reflection layer (§1), enforced, not
    /// just documented</b>: <see cref="HashReflected"/> only walks the
    /// root object's own type's <see cref="FoldbackHashAttribute"/>-tagged
    /// members. An untagged member is invisible to the walker. Once a
    /// tagged member is reached, everything reachable beneath it is
    /// walked without needing its own type separately tagged — matching
    /// the Bevy and Unreal bindings' reflective walkers, so the opt-in
    /// model reads the same across every engine this project supports.</para>
    ///
    /// <para>Reflection is an alternate <i>producer</i> feeding the same
    /// <see cref="FoldbackSession.HashField"/> sink the manual API uses,
    /// not a second bisection code path.</para>
    public static class FoldbackReflection
    {
        /// Cycles/shared references (§3): a runaway or self-referential
        /// object graph fails loudly at this depth rather than recursing
        /// forever or overflowing the stack. Matches the Bevy walker's
        /// default. Unlike Bevy's owned Rust value trees, C# reference
        /// types genuinely can form real cycles (two objects holding
        /// references to each other) — <see cref="HashReflected"/> also
        /// carries an identity-based visited-set for exactly that case,
        /// which the Bevy walker deliberately does *not* have (a
        /// pointer-identity check is unsound for Rust's value semantics;
        /// see `crates/foldback-rs/src/bevy.rs`). C#'s real reference
        /// identity makes the same technique sound here.
        public const int DefaultMaxDepth = 8;

        /// Hashes `root`'s <see cref="FoldbackHashAttribute"/>-tracked
        /// members (and everything reachable beneath them), recording
        /// each leaf as a Level-3 field hash under `entityId` at `tick`.
        /// Field names in the recorded frames are dotted/indexed paths
        /// from `fieldNamePrefix` (`"unit.pos.x"`, `"unit.items[2]"`) so
        /// bisection output stays readable. Returns the `(path, hash)`
        /// pairs actually recorded — the data a visibility tool (an
        /// editor live view, or a caller's own logging) needs to show
        /// what was captured, per §4's "close the loop" guard against
        /// invisible auto-hashing.
        public static List<(string Path, ulong Hash)> HashReflected(
            FoldbackSession session,
            ulong tick,
            ulong entityId,
            string fieldNamePrefix,
            object root)
        {
            if (session == null) throw new ArgumentNullException(nameof(session));
            if (root == null) throw new ArgumentNullException(nameof(root));

            var preview = new List<(string, ulong)>();
            var visited = new HashSet<object>(ReferenceComparer.Instance);

            foreach (var member in GetTrackedMembers(root.GetType()))
            {
                var value = member.Getter(root);
                var path = JoinPath(fieldNamePrefix, member.Name);
                Walk(session, tick, entityId, path, value, 1, visited, preview);
            }

            return preview;
        }

        /// The visibility-tooling data source (§4): every instance
        /// field/property on `type`, split into those tagged
        /// <see cref="FoldbackHashAttribute"/> and those not — the "did
        /// you mean to include this one too" check. Pure data; an editor
        /// inspector window or any other UI renders it, this method
        /// doesn't assume one.
        public static (List<string> Tracked, List<string> Untracked) ListTracked(Type type)
        {
            if (type == null) throw new ArgumentNullException(nameof(type));

            var tracked = new List<string>();
            var untracked = new List<string>();
            foreach (var member in GetAllInstanceMembers(type))
            {
                if (HasFoldbackHashAttribute(member))
                {
                    tracked.Add(member.Name);
                }
                else
                {
                    untracked.Add(member.Name);
                }
            }
            return (tracked, untracked);
        }

        private static void Walk(
            FoldbackSession session,
            ulong tick,
            ulong entityId,
            string path,
            object value,
            int depth,
            HashSet<object> visited,
            List<(string, ulong)> preview)
        {
            if (depth > DefaultMaxDepth)
            {
                throw new FoldbackReflectionException(
                    $"reflective hash walk exceeded max depth {DefaultMaxDepth} at '{path}' " +
                    "— likely a cyclic or self-referential reflected object graph");
            }

            if (value == null)
            {
                Record(session, tick, entityId, path, new byte[] { 0xFF }, preview);
                return;
            }

            var type = value.GetType();

            // Only reference types can form a genuine cycle (two objects
            // holding references to each other) — a struct is copied by
            // value, so tracking its identity would be meaningless (and
            // every boxed copy would spuriously look "new").
            if (!type.IsValueType)
            {
                if (!visited.Add(value))
                {
                    throw new FoldbackReflectionException(
                        $"reflective hash walk found a reference cycle at '{path}'");
                }
                try
                {
                    WalkTyped(session, tick, entityId, path, value, type, depth, visited, preview);
                }
                finally
                {
                    visited.Remove(value);
                }
            }
            else
            {
                WalkTyped(session, tick, entityId, path, value, type, depth, visited, preview);
            }
        }

        private static void WalkTyped(
            FoldbackSession session,
            ulong tick,
            ulong entityId,
            string path,
            object value,
            Type type,
            int depth,
            HashSet<object> visited,
            List<(string, ulong)> preview)
        {
            if (TryLeafBytes(value, type, out var leafBytes))
            {
                Record(session, tick, entityId, path, leafBytes, preview);
                return;
            }

            // §3's sorted-container rule: a set has no guaranteed
            // iteration order (checked before the generic IEnumerable
            // branch below, since a set is also enumerable and would
            // otherwise be treated as ordered).
            if (IsSet(type, out var setItemType))
            {
                var items = ((IEnumerable)value).Cast<object>()
                    .Select(item => (Text: RenderKey(item), Item: item))
                    .OrderBy(entry => entry.Text, StringComparer.Ordinal)
                    .ToList();
                _ = setItemType;
                foreach (var (text, item) in items)
                {
                    Walk(session, tick, entityId, $"{path}{{{text}}}", item, depth + 1, visited, preview);
                }
                return;
            }

            if (value is IDictionary dict)
            {
                var entries = dict.Keys.Cast<object>()
                    .Select(key => (Key: key, Text: RenderKey(key)))
                    .OrderBy(entry => entry.Text, StringComparer.Ordinal)
                    .ToList();
                foreach (var (key, text) in entries)
                {
                    Walk(session, tick, entityId, $"{path}[{text}]", dict[key], depth + 1, visited, preview);
                }
                return;
            }

            if (value is IEnumerable enumerable && !(value is string))
            {
                // Declaration/insertion order is already the game's own
                // deterministic order — no sort needed here (mirrors the
                // Bevy walker's `List`/`Array` handling).
                var i = 0;
                foreach (var item in enumerable)
                {
                    Walk(session, tick, entityId, $"{path}[{i}]", item, depth + 1, visited, preview);
                    i++;
                }
                return;
            }

            // A compound object: everything reachable from it is walked
            // unconditionally (the opt-in check already happened, either
            // at the root call or — for this recursive call — was
            // deliberately skipped, per the class doc's "once tagged,
            // walk everything beneath" model).
            foreach (var member in GetFullMembers(type))
            {
                var childValue = member.Getter(value);
                Walk(session, tick, entityId, $"{path}.{member.Name}", childValue, depth + 1, visited, preview);
            }
        }

        private static void Record(
            FoldbackSession session,
            ulong tick,
            ulong entityId,
            string path,
            byte[] bytes,
            List<(string, ulong)> preview)
        {
            session.HashField(tick, entityId, path, bytes);
            preview.Add((path, FoldbackHashBytes(bytes)));
        }

        // A local, allocation-cheap re-implementation of the same xxh3
        // hash `FoldbackSession.HashField` computes natively — used only
        // to populate the returned preview list without a second FFI
        // round-trip. Kept intentionally simple (FNV-1a) rather than
        // re-deriving the exact native hash: the *preview*'s job is to
        // show "this changed" between two calls with the same content,
        // which any stable hash achieves — the authoritative hash for
        // bisection is always the one the native session recorded.
        private static ulong FoldbackHashBytes(byte[] bytes)
        {
            const ulong offset = 14695981039346656037UL;
            const ulong prime = 1099511628211UL;
            var hash = offset;
            foreach (var b in bytes)
            {
                hash ^= b;
                hash *= prime;
            }
            return hash;
        }

        private static bool TryLeafBytes(object value, Type type, out byte[] bytes)
        {
            switch (value)
            {
                case bool v: bytes = new[] { (byte)(v ? 1 : 0) }; return true;
                case byte v: bytes = new[] { v }; return true;
                case sbyte v: bytes = new[] { unchecked((byte)v) }; return true;
                case short v: bytes = BitConverter.GetBytes(v); return true;
                case ushort v: bytes = BitConverter.GetBytes(v); return true;
                case int v: bytes = BitConverter.GetBytes(v); return true;
                case uint v: bytes = BitConverter.GetBytes(v); return true;
                case long v: bytes = BitConverter.GetBytes(v); return true;
                case ulong v: bytes = BitConverter.GetBytes(v); return true;
                case float v: bytes = BitConverter.GetBytes(v); return true;
                case double v: bytes = BitConverter.GetBytes(v); return true;
                case char v: bytes = BitConverter.GetBytes(v); return true;
                case string v: bytes = Encoding.UTF8.GetBytes(v); return true;
            }

            if (type.IsEnum)
            {
                // C# enums carry no payload (unlike a Rust enum variant),
                // so there's no "discriminant + payload" split to encode
                // — the underlying integral value alone is the fixed,
                // cross-platform-stable encoding §3 asks for.
                var underlying = Convert.ChangeType(value, Enum.GetUnderlyingType(type));
                bytes = TryLeafBytes(underlying, underlying.GetType(), out var underlyingBytes)
                    ? underlyingBytes
                    : BitConverter.GetBytes(Convert.ToInt64(value));
                return true;
            }

            bytes = null;
            return false;
        }

        private static bool IsSet(Type type, out Type itemType)
        {
            foreach (var iface in type.GetInterfaces())
            {
                if (iface.IsGenericType && iface.GetGenericTypeDefinition() == typeof(ISet<>))
                {
                    itemType = iface.GetGenericArguments()[0];
                    return true;
                }
            }
            itemType = null;
            return false;
        }

        private static string RenderKey(object key) => key?.ToString() ?? "null";

        private static string JoinPath(string prefix, string member) =>
            string.IsNullOrEmpty(prefix) ? member : $"{prefix}.{member}";

        // ---- reflection member caches (plan §2.2: raw per-tick
        // reflection is too slow, cache compiled accessors per type the
        // first time it's seen) ----

        private readonly struct MemberAccessor
        {
            public readonly string Name;
            public readonly Func<object, object> Getter;

            public MemberAccessor(string name, Func<object, object> getter)
            {
                Name = name;
                Getter = getter;
            }
        }

        private static readonly ConcurrentDictionary<Type, MemberAccessor[]> TrackedCache = new ConcurrentDictionary<Type, MemberAccessor[]>();
        private static readonly ConcurrentDictionary<Type, MemberAccessor[]> FullCache = new ConcurrentDictionary<Type, MemberAccessor[]>();

        private static MemberAccessor[] GetTrackedMembers(Type type) =>
            TrackedCache.GetOrAdd(type, t => GetAllInstanceMembers(t)
                .Where(HasFoldbackHashAttribute)
                .Select(m => new MemberAccessor(m.Name, BuildGetter(m, t)))
                .ToArray());

        /// Members walked unconditionally once inside a tracked field —
        /// scoped to *public* instance fields/properties only (unlike
        /// `GetTrackedMembers`, which also checks non-public members for
        /// the attribute). This is a deliberate, Unity-specific choice:
        /// an implicit full-recursion pass through every private/backing
        /// field of an arbitrary compound type (which, in a Unity
        /// project, can easily be a large engine type) is noisier and
        /// more surprising than walking its public surface only.
        private static MemberAccessor[] GetFullMembers(Type type) =>
            FullCache.GetOrAdd(type, t => GetPublicInstanceMembers(t)
                .Select(m => new MemberAccessor(m.Name, BuildGetter(m, t)))
                .ToArray());

        private static IEnumerable<MemberInfo> GetAllInstanceMembers(Type type)
        {
            const BindingFlags flags = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance;
            foreach (var field in type.GetFields(flags))
            {
                yield return field;
            }
            foreach (var prop in type.GetProperties(flags))
            {
                if (prop.CanRead && prop.GetIndexParameters().Length == 0)
                {
                    yield return prop;
                }
            }
        }

        private static IEnumerable<MemberInfo> GetPublicInstanceMembers(Type type)
        {
            const BindingFlags flags = BindingFlags.Public | BindingFlags.Instance;
            foreach (var field in type.GetFields(flags))
            {
                yield return field;
            }
            foreach (var prop in type.GetProperties(flags))
            {
                if (prop.CanRead && prop.GetIndexParameters().Length == 0)
                {
                    yield return prop;
                }
            }
        }

        private static bool HasFoldbackHashAttribute(MemberInfo member) =>
            member.GetCustomAttribute<FoldbackHashAttribute>() != null;

        private static Func<object, object> BuildGetter(MemberInfo member, Type declaringType)
        {
            var objParam = Expression.Parameter(typeof(object), "obj");
            var typed = Expression.Convert(objParam, declaringType);
            Expression access = member is FieldInfo field
                ? Expression.Field(typed, field)
                : Expression.Property(typed, (PropertyInfo)member);
            var boxed = Expression.Convert(access, typeof(object));
            return Expression.Lambda<Func<object, object>>(boxed, objParam).Compile();
        }

        private sealed class ReferenceComparer : IEqualityComparer<object>
        {
            public static readonly ReferenceComparer Instance = new ReferenceComparer();
            bool IEqualityComparer<object>.Equals(object x, object y) => ReferenceEquals(x, y);
            int IEqualityComparer<object>.GetHashCode(object obj) => RuntimeHelpers.GetHashCode(obj);
        }
    }

    /// Thrown when a reflective hash walk hits its depth guard or finds a
    /// genuine reference cycle — a clear, loud failure rather than a hang
    /// or a stack overflow (foldback-reflective-hashing.md §3).
    public sealed class FoldbackReflectionException : Exception
    {
        internal FoldbackReflectionException(string message) : base(message) { }
    }
}
